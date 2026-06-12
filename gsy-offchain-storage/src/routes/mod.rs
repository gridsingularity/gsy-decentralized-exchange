mod grid_topology;
mod health_check;
mod market;
mod orders;
mod profiles;
mod trades;

pub use grid_topology::*;
pub use health_check::*;
pub use market::*;
pub use orders::*;
pub use profiles::*;
pub use trades::*;

use actix_web::HttpResponse;
use chrono::DateTime;


pub fn validate_start_end_time(start_time: Option<String>, end_time: Option<String>) -> Result<(), HttpResponse> {
    let (start_time, end_time) = match (start_time, end_time) {
        (Some(start_time), Some(end_time)) => (start_time, end_time),
        _ => return Ok(()),
    };
    let start = DateTime::parse_from_rfc3339(&start_time)
        .map_err(|_| HttpResponse::BadRequest()
            .body("start_time and end_time must be valid datetimes"))?;
    let end = DateTime::parse_from_rfc3339(&end_time)
        .map_err(|_| HttpResponse::BadRequest()
            .body("start_time and end_time must be valid datetimes"))?;

    if end < start {
        return Err(HttpResponse::BadRequest()
            .body("end_time must be after start_time"));
    }
    Ok(())
}