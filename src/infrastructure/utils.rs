use chrono::{DateTime, NaiveDateTime};

pub fn date_to_ts(date: String) -> f64 {
    if let Ok(dt) = DateTime::parse_from_rfc3339(&*date) {
        //println!("parse_from_rfc3339 {}\n", date);
        return dt.timestamp_millis() as f64;
    }
    //println!("parse_from_str {}\n", date);
    let naive = NaiveDateTime::parse_from_str(&*date, "%Y-%m-%dT%H:%M:%S").unwrap();
    naive.and_utc().timestamp_millis() as f64
}

pub fn round2(val: f64) -> f64 {
    let rounded = (val * 100.0).round() / 100.0;
    sanitize_zero(rounded)
}

//remove os -0.0 para ficar 0.0
fn sanitize_zero(val: f64) -> f64 {
    if val == 0.0 {
        0.0
    } else {
        val
    }
}
