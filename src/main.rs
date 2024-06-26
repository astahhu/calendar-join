use std::collections::HashMap;
use std::env;
use std::io::Read;
use std::ops::Deref;
use std::time::Duration;

use std::path::Path;
use actix_web::http::header;
use actix_web::web::Query;
use actix_web::HttpResponseBuilder;
use anyhow::anyhow;
use reqwest::StatusCode;
use std::fs::File;
use cache::TimedCache;
use clap::builder::TypedValueParser;
use clap::Parser;
use clap::Subcommand;
use lazy_static::lazy_static;
use reqwest::Url;
use icalendar::{Calendar, parser};
use serde::Deserialize;
use actix_web::{get, web, App, HttpServer, Responder};
mod cache;
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    ///Merges the given calendar files into one calendar with the given Name (One Calendar in the resulting file)
    Merge{name: String, calendar_urls: Vec<String>},
    ///Appends the given calendar links into one File (Multiple Calendars in the resulting file)
    Append{calendar_urls: Vec<String>},

}


#[derive(Deserialize)]
#[serde(transparent)]
struct Config {
    cals: HashMap<String, HashMap<String, String>> 
}

struct CalendarMap(HashMap<String, TimedCache<anyhow::Result<Vec<Calendar>>>>);

impl CalendarMap {
    fn new(cals: HashMap<String, HashMap<String, String>>) -> Self {
        let mut map = HashMap::new();

        for (name, cal) in cals.into_iter() {
            map.insert(name, TimedCache::with_generator(move || {
                let cal = cal.clone();
                Box::pin(async move {
                    get_calendar(&cal).await
                }
            )}, Duration::from_secs(3600)));
        }
        CalendarMap(map)
    }

    async fn get<'a>(&'a self, name: &str) -> Option<impl Deref<Target = anyhow::Result<Vec<Calendar>>> + 'a > {
        Some(self.0.get(name)?.try_get().await)
    }
}

async fn get_calendar(cals: &HashMap<String, String>) -> anyhow::Result<Vec<Calendar>> {
    let mut res = vec![];

    for (name, url) in cals {
        let mut cal = icalendar::Calendar::from(
            parser::read_calendar(
                &parser::unfold(
                    &reqwest::get(url.as_str()).await?.text().await?)
                ).map_err(|err| anyhow!(""))?);

        

        cal.name(name.as_str());
        res.push(cal);
    }

    Ok(res)
}

#[derive(Deserialize)]
struct CalName {
    #[serde(default = "_index")]
    name: String
}

fn _index() -> String {
    "index".to_string()
}

lazy_static!{
    static ref CALENDAR_MAP: CalendarMap = {
        let mut buffer = String::new();
        let mut path = env::current_dir().unwrap();
        path.push("config.json");
        File::open(path.clone())
            .expect(&format!("Could not open {}.", path.display()))
            .read_to_string(&mut buffer).unwrap();

        let config : Config = serde_json::from_str(&buffer).unwrap();

        CalendarMap::new(config.cals)
    };
}

#[get("/")]
async fn merged(name: Query<CalName>) -> impl Responder {
   let Some(cal) = CALENDAR_MAP.get(&name.name).await else {
        return HttpResponseBuilder::new(StatusCode::NOT_FOUND)
            .body("Could not find Calendar")
    };
    let mut cal = match cal.as_ref() {
        Ok(cal) => cal.iter().fold(Calendar::new(), |mut a, b| {a.extend(b.components.clone()); a}),
        Err(err) => {
            return HttpResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Server Error");
        }
    };

    cal.name(&name.name);

    let body = cal.to_string();

    HttpResponseBuilder::new(StatusCode::OK)
        .content_type("text/calendar")
        .insert_header((header::CONTENT_DISPOSITION, format!("attachment; filename={}.ics",&name.name)))
        .body(body)
 
}

#[get("/appended")]
async fn appended(name: Query<CalName>) -> impl Responder {
    let Some(cal) = CALENDAR_MAP.get(&name.name).await else {
        return HttpResponseBuilder::new(StatusCode::NOT_FOUND)
            .body("Could not find Calendar")
    };
    let body = match cal.as_ref() {
        Ok(cal) => cal.iter().map(|cal| cal.to_string()).fold(String::new(), |a, b| a + &b),
        Err(err) => {
            return HttpResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR)
                .body("Internal Server Error");
        }
    };

    HttpResponseBuilder::new(StatusCode::OK)
        .content_type("text/calendar")
        .insert_header((header::CONTENT_DISPOSITION, format!("attachment; filename={}.ics",&name.name)))
        .body(body)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(appended).service(merged))
        .bind(("0.0.0.0", 8080))?
        .run()
        .await
}

