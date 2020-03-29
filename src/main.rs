use chrono::{NaiveDate, NaiveTime};

use mysql::*;

use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Debug)]
struct Calendar {
    id: i32,
    title: String,
    date: NaiveDate,
    start_time: NaiveTime,
    end_time: NaiveTime,
    practice: bool,
    notes: String,
}

async fn get_refresh_token() -> Option<MyValue> {
    let mut return_token = None;
    for row in get_all_rows("admin", false).await {
        let token = row[4].clone();
        if from_value::<String>(token.clone()) != String::new() {
            return_token = Some(MyValue::from(token));
            break;
        }
    }
    return_token
}

async fn get_access_token() -> Option<String> {
    let refresh_token = get_refresh_token();
    Some(gmail::get_access_token(&from_value::<String>(refresh_token.await?.get())).await)
}

async fn get_articles() -> Option<String> {
    let mut expiry = 0;
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let articles = get_some_like("articles", "id, title, expiry", "send_email", "1").await;
    let article = articles
        .iter()
        .filter(|row| {
            let this_expiry = from_value::<NaiveDate>(row[2].clone())
                .and_hms(0, 0, 0)
                .timestamp();
            if this_expiry > expiry && current_time < this_expiry as u64 {
                expiry = this_expiry;
                true
            } else {
                false
            }
        })
        .collect::<Vec<_>>()
        .pop()?;
    let id: i32 = from_value(article[0].clone());
    change_row_where("articles", "id", &id.to_string(), "send_email", "0").await;
    Some(from_value(article[1].clone()))
}

async fn send_article_email(email_list: Vec<String>, access_token: String, title: Option<String>) {
    if let Some(title) = title {
        gmail::send_email(email_list, &title, &format!("Hi everyone,\r\nThe {} are available on the Children's Choir website at olmmcc [dot] tk. Thank you and God Bless!\r\n\r\nJustus", title), &access_token).await;
    }
}

async fn send_calendar_email(email_list: Vec<String>, access_token: String, events: Vec<Calendar>) {
    if !events.is_empty() {
        let mut email = "Hi everybody, \r\nThe upcoming choir practices will be on day-time, and on day-time. Also, the upcoming event will be on day-time".to_string();

        println!("{:?}", events);
        //println!("{:?}, {:?}", titles, dates);
        email = format!("{}\r\nThank you and God Bless!\r\n\r\nJustus", email);
        gmail::send_email(email_list, "Upcoming Children's Choir Events", &email, &access_token).await;
    }
}

async fn get_email_list() -> Vec<String> {
    let mut email_list = get_like("users", "subscription_policy", "1").await;
    email_list.append(&mut get_like("users", "subscription_policy", "2").await);
    email_list.append(&mut get_like("admin", "subscription_policy", "1").await);
    email_list.append(&mut get_like("admin", "subscription_policy", "2").await);
    let mut email_list: Vec<_> = email_list
        .iter()
        .map(|x| from_value(x[0].clone()))
        .collect();
    email_list.sort();
    email_list.dedup();
    email_list
}

async fn get_calendar_events() -> Vec<Calendar> {
    let events = get_like("calendar", "send_email", "1").await;
    let events: Vec<Calendar> = events
        .iter()
        .map(|x| Calendar {
            id: from_value(x[0].clone()),
            title: from_value(x[1].clone()),
            date: from_value(x[2].clone()),
            start_time: from_value(x[3].clone()),
            end_time: from_value(x[4].clone()),
            practice: from_value(x[5].clone()),
            notes: from_value(x[6].clone()),
        })
        .collect();
    for event in events.iter() {
        change_row_where("articles", "id", &event.id.to_string(), "send_email", "0").await;
    }
    events
}

async fn emaild() -> Option<()> {
    let (access_token, title, email_list, events) = futures::join!(
        get_access_token(),
        get_articles(),
        get_email_list(),
        get_calendar_events()
    );
    let access_token = access_token?;
    futures::join!(
        send_article_email(email_list.clone(), access_token.clone(), title),
        send_calendar_email(email_list, access_token, events)
    );
    Some(())
}

#[tokio::main]
async fn main() {
    if let None = emaild().await {
        println!("Error");
    }
}
