use actix_web::http::StatusCode;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use md5::digest::consts::False;
use rand::{distributions::Alphanumeric, Rng};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

struct AppStateWithCounter {
    db_connection: Mutex<sqlite::Connection>,
}

fn is_link_added(key: &String, db: &sqlite::Connection) -> Option<String> {
    let query = format!("SELECT hash FROM link_to_hash WHERE link=\"{}\"", key);
    let mut result = None;
    db.iterate(query, |pairs| {
        for (_k, v) in pairs.iter() {
            if let Some(finding) = v {
                result = Some(finding.to_string());
            }
        }
        true
    })
    .unwrap();
    result
}

fn is_hash_added(key: &String, db: &sqlite::Connection) -> bool {
    let query = format!("SELECT hash FROM link_to_hash WHERE hash=\"{}\"", key);
    let mut is_added = false;
    db.iterate(query, |_pairs| {
        is_added = true;
        true
    })
    .unwrap();
    is_added
}

fn update(hash: &String, link: &String, db: &sqlite::Connection) {
    let since_the_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let query: String = format!(
        "INSERT OR IGNORE INTO link_to_hash VALUES(\"{}\",\"{}\", {});
         UPDATE link_to_hash SET timestamp={} WHERE hash=\"{}\"",
        hash,
        link,
        since_the_epoch.as_millis(),
        since_the_epoch.as_millis(),
        hash
    );
    println!("Encoded {:?} to {}", link, hash);
    db.execute(query).unwrap();
}

fn generate_key() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect()
}

#[get("/error")]
async fn error() -> impl Responder {
    HttpResponse::NotFound().body("No url found for given hash")
}

#[get("/")]
async fn start_page() -> impl Responder {
    HttpResponse::Ok().body("Hello")
}

#[get("/{hash}")]
async fn decode_url(
    hash: web::Path<String>,
    data: web::Data<AppStateWithCounter>,
) -> impl Responder {
    let connection = data.db_connection.lock().unwrap();
    let url_hash = hash.into_inner();
    let mut result = None;
    connection
        .iterate(
            format!("SELECT link from link_to_hash WHERE hash=\"{}\"", url_hash),
            |pairs| {
                for (_k, v) in pairs.iter() {
                    if let Some(hash) = v {
                        result = Some(hash.to_string());
                    }
                }
                true
            },
        )
        .unwrap();
    println!("Found {:?} link for {}", result, url_hash);
    if let Some(url) = result {
        return actix_web::web::Redirect::to(url).permanent();
    }
    actix_web::web::Redirect::to("/error").permanent()
}

#[post("/encode")]
async fn encode_url(link: String, data: web::Data<AppStateWithCounter>) -> impl Responder {
    let connection = data.db_connection.lock().unwrap();

    if let Some(hash) = is_link_added(&link, &connection) {
        update(&hash, &link, &connection);
        return HttpResponse::Ok().body(hash);
    }
    let mut hash: String = generate_key();
    while is_hash_added(&hash, &connection) {
        hash = generate_key();
    }
    update(&hash, &link, &connection);
    HttpResponse::Ok().body(hash)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let connection = sqlite::open("link_storage.db").unwrap();

    let query = "CREATE TABLE IF NOT EXISTS `link_to_hash` (hash TEXT NOT NULL, link TEXT NOT NULL, timestamp BIG INT, PRIMARY KEY(`hash`));";

    connection.execute(query).unwrap();

    let data_base = web::Data::new(AppStateWithCounter {
        db_connection: Mutex::new(connection),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(data_base.clone())
            .service(start_page)
            .service(error)
            .service(decode_url)
            .service(encode_url)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
