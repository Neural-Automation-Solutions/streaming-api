#[macro_use]
extern crate dotenv_codegen;

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use std::fs::File;
use std::io::Write;
use actix_web::{get, post, web, App, Error, HttpResponse, HttpServer, Responder};
use actix_web::http::header::CONTENT_TYPE;
use futures::stream::unfold;
use serde::Deserialize;

struct AppState {
    last_frames: RwLock<HashMap<String, Vec<u8>>>,
    save_frames: RwLock<HashMap<String, bool>>,
    frame_indexes: RwLock<HashMap<String, usize>>,
}


#[derive(Deserialize)]
struct ToggleRequest {
    toggle: bool,
}

#[get("/test")]
async fn test() -> impl Responder {
    HttpResponse::Ok().body("Hello, yiannis!")
}

#[post("/v1/stream/set_save/{name}")]
async fn set_save_frames(name: web::Path<String>, data: web::Data<AppState>, toggle_request: web::Json<ToggleRequest>) -> impl Responder {
    let mut map = data.save_frames.write().unwrap();
    let name = name.into_inner();
    
    // update save_frames map
    map.insert(name, toggle_request.toggle);

    let message = if toggle_request.toggle {
        "Frames will be saved"
    } else {
        "Frames will not be saved"
    };

    HttpResponse::Ok().body(message)
}

#[post("/v1/stream/{name}")]
async fn save_last_frame(name: web::Path<String>, data: web::Data<AppState>, body: web::Bytes) -> impl Responder {
    let mut map = data.last_frames.write().unwrap();
    let name = name.into_inner();
    let name_str = name.as_str();

    // save frame to memory
    map.insert(name.clone(), body.to_vec());
    
    // save frame to disk
    if let Some(save) = data.save_frames.read().unwrap().get(name_str) {
        if *save {
            let mut frame_indexes = data.frame_indexes.write().unwrap();
            let index = frame_indexes.entry(name.clone()).or_insert(0);
            
            // create a directory for this stream if there is none
            std::fs::create_dir_all(format!("{}/{}", dotenv!("FRAME_SAVE_PATH"), name)).unwrap();

            let filepath = format!("{}/{}/{}_{}.jpeg", dotenv!("FRAME_SAVE_PATH"), name, name, format!("{:0>7}", index));
            
            let mut file = match File::create(filepath) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error creating file: {}", e);
                    return HttpResponse::InternalServerError().finish();
                }
            };

            // write frame to file
            if let Err(e) = file.write_all(&body) {
                eprintln!("Error writing to file: {}", e);
                return HttpResponse::InternalServerError().finish();
            }

            // increment frame index
            *index += 1;
        }
    }

    HttpResponse::Ok().body("Last frame saved")
}

#[get("/v1/stream/{name}.mjpg")]
async fn stream_frame(name: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let boundary = "--FRAME";

    let stream_body = unfold(0, move |_| {
        let data = data.clone();
        let name = name.clone();
        async move {
            // simulate time delay between frames
            // removing this line ** doesn't ** make the stream faster
            // it actually becomes very laggy
            tokio::time::sleep(Duration::from_millis(1)).await;
            
            let name_str = name.as_str();
            if let Some(last_frame) = data.last_frames.read().unwrap().clone().get(name_str) {

                // frame for this stream is available

                let content_length = last_frame.len();
                let headers = format!(
                    "\r\n{}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    boundary, content_length
                );
    
                let mut frame = Vec::new();
                frame.extend_from_slice(headers.as_bytes());
                frame.extend_from_slice(&last_frame);
                frame.extend_from_slice(b"\r\n");
    
                return Some((Ok::<_, Error>(web::Bytes::from(frame)), 0))
            }

            // frame for this stream is not available, return Error
            Some((Err::<_, Error>(actix_web::error::ErrorNotFound("Frame not found")), 0))
        }
    });

    HttpResponse::Ok()
        .insert_header((CONTENT_TYPE, format!("multipart/x-mixed-replace;boundary={}", boundary)))
        .streaming(Box::pin(stream_body))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app_state = web::Data::new(AppState {
        last_frames: RwLock::new(HashMap::new()),
        save_frames: RwLock::new(HashMap::new()),
        frame_indexes: RwLock::new(HashMap::new()),
    });
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(save_last_frame)
            .service(stream_frame)
            .service(set_save_frames)
            .service(test)
    })
    .bind(format!("0.0.0.0:{}", dotenv!("PORT")))?
    .run()
    .await
}
