//! Forms Demo Example
//!
//! Demonstrates the acton-htmx form builder API with HTMX integration.
//!
//! Run with: `cargo run --example forms_demo`

use acton_htmx::prelude::*;
use axum::{
    extract::Form,
    response::Html,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use validator::Validate;

// =============================================================================
// Form Data Structures
// =============================================================================

#[derive(Debug, Deserialize, Validate)]
struct LoginForm {
    #[validate(email(message = "Please enter a valid email address"))]
    email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    password: String,
    remember: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
struct RegisterForm {
    #[validate(length(min = 2, message = "Name must be at least 2 characters"))]
    name: String,
    #[validate(email(message = "Please enter a valid email address"))]
    email: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    password: String,
    country: String,
    #[allow(dead_code)]
    bio: Option<String>,
    #[allow(dead_code)]
    terms: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchForm {
    query: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// Build the HTML page shell
fn html_page(login_form: &str, register_form: &str, search_form: &str) -> String {
    let css = r"
        body { font-family: system-ui, sans-serif; max-width: 800px; margin: 2rem auto; padding: 0 1rem; }
        h1, h2 { color: #333; }
        .demo-section { margin: 2rem 0; padding: 1rem; border: 1px solid #ddd; border-radius: 8px; }
        .form-group { margin: 1rem 0; }
        .form-label { display: block; margin-bottom: 0.5rem; font-weight: 500; }
        .form-input { width: 100%; padding: 0.5rem; border: 1px solid #ccc; border-radius: 4px; box-sizing: border-box; }
        .form-input:focus { outline: none; border-color: #007bff; box-shadow: 0 0 0 2px rgba(0,123,255,0.25); }
        .form-input-error { border-color: #dc3545; }
        .form-error { display: block; color: #dc3545; font-size: 0.875rem; margin-top: 0.25rem; }
        .form-help { display: block; color: #6c757d; font-size: 0.875rem; margin-top: 0.25rem; }
        .form-submit { background: #007bff; color: white; padding: 0.75rem 1.5rem; border: none; border-radius: 4px; cursor: pointer; font-size: 1rem; }
        .form-submit:hover { background: #0056b3; }
        .form-submit:disabled { background: #ccc; cursor: not-allowed; }
        #result, #search-results { margin-top: 1rem; padding: 1rem; background: #f8f9fa; border-radius: 4px; }
        .htmx-request .htmx-indicator { display: inline-block !important; }
        .htmx-indicator { display: none; }
        textarea.form-input { resize: vertical; }
        nav a { margin-right: 1rem; color: #007bff; text-decoration: none; }
        nav a:hover { text-decoration: underline; }
    ";

    format!(
        r##"<!DOCTYPE html>
<html>
<head>
    <title>acton-htmx Forms Demo</title>
    <script src="https://unpkg.com/htmx.org@2.0.4"></script>
    <style>{css}</style>
</head>
<body>
    <h1>acton-htmx Forms Demo</h1>
    <nav>
        <a href="#login-section">Login Form</a>
        <a href="#register-section">Registration Form</a>
        <a href="#search-section">Live Search</a>
    </nav>

    <div class="demo-section" id="login-section">
        <h2>Login Form</h2>
        <p>A simple login form with HTMX submission.</p>
        <div id="login-form-container">
            {login_form}
        </div>
        <div id="login-result"></div>
    </div>

    <div class="demo-section" id="register-section">
        <h2>Registration Form</h2>
        <p>A more complex form with validation and multiple field types.</p>
        <div id="register-form-container">
            {register_form}
        </div>
        <div id="register-result"></div>
    </div>

    <div class="demo-section" id="search-section">
        <h2>Live Search</h2>
        <p>Demonstrates live search with debounced input.</p>
        <div id="search-form-container">
            {search_form}
        </div>
        <div id="search-results">Type to search...</div>
    </div>
</body>
</html>"##
    )
}

/// Index page with links to form demos
async fn index() -> Html<String> {
    Html(html_page(
        &build_login_form(None),
        &build_register_form(None),
        &build_search_form(),
    ))
}

/// Build the login form
fn build_login_form(errors: Option<&ValidationErrors>) -> String {
    let mut builder = FormBuilder::new("/login", "POST")
        .id("login-form")
        .csrf_token("demo_csrf_token")
        .htmx_post("/login")
        .htmx_target("#login-result")
        .htmx_swap("innerHTML")
        .htmx_indicator("#login-spinner");

    if let Some(e) = errors {
        builder = builder.errors(e);
    }

    let form = builder
        .field("email", InputType::Email)
        .label("Email Address")
        .placeholder("you@example.com")
        .required()
        .autocomplete("email")
        .done()
        .field("password", InputType::Password)
        .label("Password")
        .placeholder("Enter your password")
        .required()
        .min_length(8)
        .autocomplete("current-password")
        .done()
        .checkbox("remember")
        .label("Remember me for 30 days")
        .done()
        .submit("Sign In")
        .build();

    format!(
        r#"{form}<span id="login-spinner" class="htmx-indicator"> Loading...</span>"#
    )
}

/// Build the registration form
fn build_register_form(errors: Option<&ValidationErrors>) -> String {
    let mut builder = FormBuilder::new("/register", "POST")
        .id("register-form")
        .csrf_token("demo_csrf_token")
        .htmx_post("/register")
        .htmx_target("#register-result")
        .htmx_swap("innerHTML");

    if let Some(e) = errors {
        builder = builder.errors(e);
    }

    builder
        .field("name", InputType::Text)
        .label("Full Name")
        .placeholder("John Doe")
        .required()
        .min_length(2)
        .autocomplete("name")
        .done()
        .field("email", InputType::Email)
        .label("Email Address")
        .placeholder("you@example.com")
        .required()
        .autocomplete("email")
        .help("We'll never share your email with anyone.")
        .done()
        .field("password", InputType::Password)
        .label("Password")
        .required()
        .min_length(8)
        .autocomplete("new-password")
        .help("Must be at least 8 characters.")
        .done()
        .select("country")
        .label("Country")
        .placeholder_option("Select your country...")
        .option("us", "United States")
        .option("ca", "Canada")
        .option("uk", "United Kingdom")
        .option("au", "Australia")
        .option("de", "Germany")
        .option("fr", "France")
        .required()
        .done()
        .textarea("bio")
        .label("Bio (optional)")
        .placeholder("Tell us about yourself...")
        .rows(4)
        .done()
        .checkbox("terms")
        .label("I agree to the Terms of Service and Privacy Policy")
        .required()
        .done()
        .submit("Create Account")
        .build()
}

/// Build the live search form
fn build_search_form() -> String {
    FormBuilder::new("/search", "GET")
        .id("search-form")
        .novalidate()
        .field("query", InputType::Search)
        .label("Search")
        .placeholder("Type to search...")
        .htmx_get("/search")
        .htmx_trigger("keyup changed delay:300ms")
        .htmx_target("#search-results")
        .htmx_swap("innerHTML")
        .done()
        .build()
}

/// Handle login form submission
async fn handle_login(Form(form): Form<LoginForm>) -> Html<String> {
    // Validate the form
    match form.validate() {
        Ok(()) => {
            // Simulated login success
            Html(format!(
                r#"<div style="color: green; padding: 1rem; background: #d4edda; border-radius: 4px;">
                    <strong>Success!</strong> Welcome back, {email}.
                    <br><small>Remember me: {remember}</small>
                </div>"#,
                email = form.email,
                remember = form.remember.is_some()
            ))
        }
        Err(validation_errors) => {
            // Convert to our ValidationErrors and rebuild form
            let errors: ValidationErrors = validation_errors.into();
            let form_html = build_login_form(Some(&errors));
            Html(format!(
                r#"<div style="color: red; padding: 1rem; background: #f8d7da; border-radius: 4px; margin-bottom: 1rem;">
                    Please fix the errors below.
                </div>
                {form_html}"#
            ))
        }
    }
}

/// Handle registration form submission
async fn handle_register(Form(form): Form<RegisterForm>) -> Html<String> {
    match form.validate() {
        Ok(()) => Html(format!(
            r#"<div style="color: green; padding: 1rem; background: #d4edda; border-radius: 4px;">
                <strong>Account Created!</strong>
                <br>Welcome, {name}! A confirmation email has been sent to {email}.
                <br>Country: {country}
            </div>"#,
            name = form.name,
            email = form.email,
            country = form.country
        )),
        Err(validation_errors) => {
            let errors: ValidationErrors = validation_errors.into();
            let form_html = build_register_form(Some(&errors));
            Html(format!(
                r#"<div style="color: red; padding: 1rem; background: #f8d7da; border-radius: 4px; margin-bottom: 1rem;">
                    Please fix the errors below.
                </div>
                {form_html}"#
            ))
        }
    }
}

/// Handle live search
async fn handle_search(Form(form): Form<SearchForm>) -> Html<String> {
    let query = form.query.trim();

    if query.is_empty() {
        return Html("Type to search...".into());
    }

    // Simulated search results
    let items = vec![
        "Apple", "Banana", "Cherry", "Date", "Elderberry", "Fig", "Grape",
        "Honeydew", "Kiwi", "Lemon", "Mango", "Orange", "Papaya", "Quince",
    ];

    let matches: Vec<_> = items
        .iter()
        .filter(|item| item.to_lowercase().contains(&query.to_lowercase()))
        .collect();

    if matches.is_empty() {
        Html(format!(r#"<em>No results found for "{query}"</em>"#))
    } else {
        let list: String = matches
            .iter()
            .fold(String::new(), |mut acc, item| {
                use std::fmt::Write;
                let _ = write!(&mut acc, "<li>{item}</li>");
                acc
            });
        Html(format!(
            r#"<strong>Found {count} result(s) for "{query}":</strong><ul>{list}</ul>"#,
            count = matches.len()
        ))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Build the router
    let app = Router::new()
        .route("/", get(index))
        .route("/login", post(handle_login))
        .route("/register", post(handle_register))
        .route("/search", get(handle_search));

    // Start the server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Forms demo server running at http://127.0.0.1:3000");
    axum::serve(listener, app).await?;

    Ok(())
}
