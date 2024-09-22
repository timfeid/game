// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{
    command, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindowBuilder, Wry,
};
use tauri_plugin_store::{with_store, StoreBuilder, StoreCollection};

use bardecoder;
use image::{imageops::crop, DynamicImage, ImageBuffer, Rgba, RgbaImage};

use std::error::Error;

fn decode_qr_code(image_data: DynamicImage) -> Vec<String> {
    let decoder = bardecoder::default_decoder();

    let mut uris = vec![];
    let results = decoder.decode(&image_data);
    for result in results {
        if let Ok(otp_uri) = result {
            uris.push(otp_uri);
        }
    }

    uris
}

#[command]
async fn set_refresh_token(
    app: AppHandle<Wry>,
    token: String,
) -> Result<(), tauri_plugin_store::Error> {
    // Access the store collection
    let stores = app.try_state::<StoreCollection<Wry>>().expect("stores");
    // Define the path for the store (adjust as needed)
    let path = app
        .path()
        .app_data_dir()
        .expect("unable to find data dir")
        .join("data.json");

    // Use the store with the `with_store` helper
    with_store(app.clone(), stores, path, |store| {
        // Retrieve a value from the store
        store.insert("refresh_token".to_string(), json!(token))?;
        store.save()?;
        // Return an owned String by cloning the value
        Ok(())
    })
}

#[command]
async fn get_refresh_token(app: AppHandle<Wry>) -> Option<Value> {
    let stores = app.try_state::<StoreCollection<Wry>>().expect("stores");
    let path = app
        .path()
        .app_data_dir()
        .expect("unable to find data dir")
        .join("data.json");

    with_store(app.clone(), stores, path, |store| {
        store
            .get("refresh_token")
            .ok_or_else(|| {
                tauri_plugin_store::Error::Tauri(tauri::Error::AssetNotFound("test.".to_string()))
            })
            .cloned()
    })
    .ok()
}

#[derive(Serialize, Deserialize)]
struct Details {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl Details {
    fn new(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Details {
        Details {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        }
    }
}

#[tauri::command]
fn prep_qr(app: AppHandle<Wry>) -> Result<Details, String> {
    let window = app.get_webview_window("Third").expect("hmm");

    let position = window
        .outer_position()
        .expect("unable to get position of window");
    let size = window.outer_size().expect("unable to get size of window");
    window
        .set_position(PhysicalPosition::new(9000, 9000))
        .expect("Unable to set window opacity");

    return Ok(Details::new(position, size));
}

#[tauri::command]
fn scan_qr(app: AppHandle<Wry>, details: Details) -> Vec<String> {
    println!("capture?");

    let window = app.get_webview_window("Third").expect("hmm");
    let binding = xcap::Monitor::all().unwrap();
    let monitor = binding.first().unwrap();
    let image = monitor.capture_image().unwrap();
    println!("Screenshot taken.");

    // Convert image to a buffer
    let mut buffer =
        RgbaImage::from_raw(image.width() as u32, image.height() as u32, image.to_vec())
            .expect("Failed to create image buffer");

    let cropped_image = crop(
        &mut buffer,
        details.x as u32,
        details.y as u32,
        details.width,
        details.height,
    )
    .to_image();

    let results = decode_qr_code(cropped_image.into());
    let original_position = PhysicalPosition::new(details.x.clone(), details.y.clone());

    match results.len().eq(&0) {
        true => window.set_position(original_position),
        false => {
            window.close().expect("close the window");
            app.emit_to("tauri-app", "qr_results", &results)
        }
    }
    .expect("something went wrong moving the window");

    results
}

#[tauri::command]
fn start_qr(app: AppHandle<Wry>) {
    WebviewWindowBuilder::new(
        &app,
        "Third",
        tauri::WebviewUrl::App(Path::new("qr-scan").to_path_buf()),
    )
    .always_on_top(true)
    .transparent(true)
    .shadow(false)
    .title("Tauri - Third")
    .build()
    .unwrap();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let path = app
                .path()
                .app_data_dir()
                .expect("unable to find data dir")
                .join("data.json");

            if !path.exists() {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut file = fs::File::create(&path)?;
                file.write_all(b"{}")?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_refresh_token,
            set_refresh_token,
            scan_qr,
            prep_qr,
            start_qr
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod tests {
    use std::path::Path;

    use crate::decode_qr_code;

    #[test]
    fn test() {
        let img = image::open(Path::new("../screenshot.png")).expect("wwww");

        decode_qr_code(img);
    }
}
