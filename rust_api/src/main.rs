use dotenv::dotenv;
use postgres::Error as PostgresError;
use postgres::{Client, NoTls};
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

#[macro_use]
extern crate serde_derive;

// Model
#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
}
fn DB_URL() -> String {
    dotenv().ok();
    env::var("DATABASE_URL").unwrap()
}

const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_SERVER_ERROR: &str = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n";

fn main() {
    if let Err(e) = setup_database() {
        println!("Setup Database Error: {}", e);
        return;
    }

    let listener = TcpListener::bind(format!("0.0.0.0:8080")).unwrap();
    println!("Server started at port 8080");

    //handle the client
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connection established");
                handle_client(stream);
            }
            Err(e) => {
                println!("Connection Error: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let mut request = String::new();

    match stream.read(&mut buffer) {
        Ok(size) => {
            request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

            let (status_line, content) = match &*request {
                r if r.starts_with("POST") && r.contains("/users") => handle_post_request(r),
                r if r.starts_with("GET") && r.contains("/user/") => handle_get_request(r),
                r if r.starts_with("GET") && r.contains("/users") => handle_get_all_request(r),
                r if r.starts_with("PUT") && r.contains("/users/") => handle_put_request(r),
                r if r.starts_with("DELETE") && r.contains("/users/") => handle_delete_request(r),
                _ => (NOT_FOUND.to_string(), "Not Found URL".to_string()),
            };

            stream
                .write_all(format!("{}{}", status_line, content).as_bytes())
                .unwrap();
        }
        Err(e) => println!("Failed to read from connection: {}", e),
    }
}

/*
*  Controllers
*/

fn handle_post_request(request: &str) -> (String, String) {
    match (
        get_user_request_body(&request),
        Client::connect(DB_URL().as_str(), NoTls),
    ) {
        (Ok(user), Ok(mut client)) => {
            client
                .execute(
                    "INSERT INTO users (name, email) VALUES ($1, $2)",
                    &[&user.name, &user.email],
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User Created".to_string())
        }
        _ => (
            INTERNAL_SERVER_ERROR.to_string(),
            "Internal Server Error".to_string(),
        ),
    }
}

fn handle_get_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        Client::connect(DB_URL().as_str(), NoTls),
    ) {
        (Ok(id), Ok(mut client)) => {
            println!("ID: {}", id);
            match client.query_one("SELECT * FROM users WHERE id = $1", &[&id]) {
                Ok(row) => {
                    let user = User {
                        id: row.get(0),
                        name: row.get(1),
                        email: row.get(2),
                    };
                    (
                        OK_RESPONSE.to_string(),
                        serde_json::to_string(&user).unwrap(),
                    )
                }
                _ => (NOT_FOUND.to_string(), "User Not Found".to_string()),
            }
        }
        _ => (
            INTERNAL_SERVER_ERROR.to_string(),
            "Internal Server Error".to_string(),
        ),
    }
}

fn handle_get_all_request(request: &str) -> (String, String) {
    match Client::connect(DB_URL().as_str(), NoTls) {
        Ok(mut client) => {
            let mut users = Vec::new();

            for row in client.query("SELECT * FROM users", &[]).unwrap() {
                users.push(User {
                    id: row.get(0),
                    name: row.get(1),
                    email: row.get(2),
                })
            }

            (
                OK_RESPONSE.to_string(),
                serde_json::to_string(&users).unwrap(),
            )
        }
        _ => (
            INTERNAL_SERVER_ERROR.to_string(),
            "Internal Server Error".to_string(),
        ),
    }
}

fn handle_put_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        get_user_request_body(&request),
        Client::connect(DB_URL().as_str(), NoTls),
    ) {
        (Ok(id), Ok(user), Ok(mut client)) => {
            client
                .execute(
                    "UPDATE users SET name = $1, email = $2 WHERE id = $3",
                    &[&user.name, &user.email, &id],
                )
                .unwrap();
            (OK_RESPONSE.to_string(), "User Updated".to_string())
        }
        _ => (
            INTERNAL_SERVER_ERROR.to_string(),
            "Internal Server Error".to_string(),
        ),
    }
}

fn handle_delete_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        Client::connect(DB_URL().as_str(), NoTls),
    ) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client
                .execute("DELETE FROM users WHERE id = $1", &[&id])
                .unwrap();

            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "User Not Found".to_string());
            }

            (OK_RESPONSE.to_string(), "User Deleted".to_string())
        }
        _ => (
            INTERNAL_SERVER_ERROR.to_string(),
            "Internal Server Error".to_string(),
        ),
    }
}

fn setup_database() -> Result<(), PostgresError> {
    println!("Database URL: {}", DB_URL());
    let mut client = Client::connect(DB_URL().as_str(), NoTls)?;

    client.batch_execute(
        "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL
        )",
    )?;
    Ok(())
}

fn get_user_request_body(request: &str) -> Result<User, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

fn get_id(request: &str) -> &str {
    request
        .split("/")
        .nth(4)
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
}
