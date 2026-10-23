//! Creates or updates a service account in the `users` collection.
//!
//! The password is read from the first line of stdin, never from arguments or
//! environment variables, and is stored as a bcrypt hash. Running it again for
//! an existing username replaces the password (rotation).
//!
//! Example:
//!
//! ```text
//! printf '%s\n' "$PW" | create-service-user launchpad-backend
//! ```

use launchpad_matching_service::auth::model::{UserModel, hash_password};
use launchpad_matching_service::configuration::get_configuration;
use std::io::BufRead;
use std::process;

fn fail(message: &str) -> ! {
    eprintln!("Error: {}", message);
    process::exit(1);
}

fn read_password() -> Result<String, std::io::Error> {
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    // Strip only the trailing line ending, keep any other characters as typed.
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    Ok(line)
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        fail("usage: create-service-user <username> (password is read from stdin)");
    }
    let username = args[1].as_str();
    if username.is_empty() || username.chars().any(char::is_whitespace) {
        fail("username must be non-empty and must not contain whitespace");
    }

    let password = match read_password() {
        Ok(p) => p,
        Err(e) => fail(&format!("failed to read password from stdin: {}", e)),
    };
    let password_hash = match hash_password(&password) {
        Ok(h) => h,
        Err(e) => fail(&e.to_string()),
    };
    drop(password);

    // UserModel::new() panics on a missing configuration, so check it first
    // to report a readable error instead.
    if let Err(e) = get_configuration() {
        fail(&format!("failed to load configuration: {}", e));
    }
    let user_model = match UserModel::new().await {
        Ok(m) => m,
        Err(e) => fail(&format!("failed to connect to MongoDB: {}", e)),
    };
    if let Err(e) = user_model.upsert_user(username, &password_hash).await {
        fail(&format!("failed to store user: {}", e));
    }

    println!("Service user '{}' created or updated.", username);
}
