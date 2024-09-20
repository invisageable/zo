use actix_web::{middleware, web, App, HttpResponse, HttpServer, Responder};

use dotenv::dotenv;
use dotenv_codegen::dotenv;

async fn index() -> impl Responder {
  let html =
    std::fs::read_to_string("./index.html").expect("Cannot read html file");

  HttpResponse::Ok()
    .content_type("text/html; charset=utf-8")
    .body(html)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  dotenv().ok();

  env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

  let _address = dotenv!("ZO_WEBSITE_IP");
  let _port = dotenv!("ZO_WEBSITE_PORT").parse::<u16>().unwrap();

  // println!("{}", dotenv!("ZO_WEBSITE_IP"));

  HttpServer::new(|| {
    App::new()
      .wrap(middleware::Logger::default())
      .service(actix_files::Files::new("/assets", ".."))
      .service(web::resource("/").route(web::get().to(index)))
  })
  .bind(("127.0.0.1", 8080))?
  .workers(1)
  .run()
  .await
}
