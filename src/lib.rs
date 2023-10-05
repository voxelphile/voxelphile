use std::{
    convert, env,
    io::{self, stdin, BufRead},
    thread, time,
};

pub mod net;

pub fn main() {
    // log::init().expect("failed to initialize logger");

    // trace!("Hello, Xenotech!");

    thread::Builder::new()
        .name("client".to_string())
        .spawn(|| {
            // trace!("Hello!");
            client();
        })
        .expect("failed to start client");

    interface();
}

fn busy_loop() {
    loop {
        thread::sleep(time::Duration::from_secs_f32(1.0));
    }
}

#[derive(Debug)]
pub enum LoginOption {
    Signup,
    Login,
}

#[derive(Debug)]
pub enum LoginError {
    BadPassword,
}

#[derive(Debug)]
pub enum SignupError {
    InvalidPassword,
}

pub struct User {}

fn interface() {
    let user = start();
}

fn start() -> User {
    let stdin = io::stdin();

    // info!("Do you have an account?");
    // info!("Type \"signup\" or \"login\"");

    let mut input = String::new();
    let login_option;

    loop {
        stdin.lock().read_line(&mut input).expect("failed to read");
        input = input.trim().to_owned();
        match input.to_lowercase().as_str() {
            "signup" => {
                login_option = LoginOption::Signup;
            }
            "login" => {
                login_option = LoginOption::Login;
            }
            _ => {
                // info!("Invalid input: \"{}\"", input);
                // input = default();
                continue;
            }
        }
        break;
    }

    match login_option {
        LoginOption::Signup => match signup() {
            Ok(user) => user,
            Err(e) => {
                // error!("{:?}", e);
                println!("");
                start()
            }
        },
        LoginOption::Login => match login() {
            Ok(user) => user,
            Err(e) => {
                // error!("{:?}", e);
                println!("");
                start()
            }
        },
    }
}

fn login() -> Result<User, LoginError> {
    Err(LoginError::BadPassword)
}

fn signup() -> Result<User, SignupError> {
    Err(SignupError::InvalidPassword)
}

fn client() {}

fn server() {}
