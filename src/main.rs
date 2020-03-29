use chrono::{Local, NaiveDate, NaiveTime};

use mysql::*;

use std::time::{SystemTime, UNIX_EPOCH};

struct Event {
    id: Option<i32>,
    title: String,
    date: Option<NaiveDate>,
    start_time: NaiveTime,
    end_time: NaiveTime,
    practice: bool,
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

async fn send_article_email(email_list: Vec<String>, access_token: &str, title: Option<String>) {
    if let Some(title) = title {
        gmail::send_email(email_list, &title, &format!("Hi everyone,\r\nThe {} are available on the Children's Choir website at olmmcc [dot] tk. Thank you and God Bless!\r\n\r\nJustus", title), access_token).await;
    }
}

async fn send_calendar_email(email_list: Vec<String>, access_token: &str, events: Vec<Event>) {
    if !events.is_empty() {
        let mut email = "Hi everybody, \r\n".to_string();
        let mut choir_practices = vec![];
        let mut other_event = None;
        for event in events {
            if event.practice {
                choir_practices.push(event);
            } else if let None = other_event {
                other_event = Some(event);
            }
        }
        if !choir_practices.is_empty() {
            let plural = if choir_practices.len() > 1 { "s" } else { "" };
            email = format!("{}This month's choir practice{} will be on ", email, plural);
            for i in 0..choir_practices.len() {
                let on_from_to = format!(
                    "{}, from {} to {}",
                    choir_practices[i].date.unwrap().format("%A, %B %-d"),
                    choir_practices[i].start_time.format("%-I:%M %p"),
                    choir_practices[i].end_time.format("%-I:%M %p")
                );
                if choir_practices.len() == 1 {
                    email = format!("{}{}.", email, on_from_to,);
                } else if i == choir_practices.len() - 1 {
                    email = format!("{}, and on {}.", email, on_from_to,);
                } else if i == 0 {
                    email = format!("{}{}", email, on_from_to);
                } else {
                    email = format!("{}, on {}", email, on_from_to,);
                }
            }
        }
        if let Some(event) = other_event {
            email = format!(
                "{} Also, the upcoming {} will be on {}, from {} to {}.",
                email,
                event.title,
                event.date.unwrap().format("%A, %B %-d"),
                event.start_time.format("%-I:%M %p"),
                event.end_time.format("%-I:%M %p")
            );
        }
        email = format!("{}\r\nThank you and God Bless!\r\n\r\nJustus", email);
        gmail::send_email(
            email_list,
            "Upcoming Children's Choir Events",
            &email,
            access_token,
        )
        .await;
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

async fn get_reminder_list() -> Vec<String> {
    let mut email_list = get_like("users", "subscription_policy", "2").await;
    email_list.append(&mut get_like("admin", "subscription_policy", "2").await);
    let mut email_list: Vec<_> = email_list
        .iter()
        .map(|x| from_value(x[0].clone()))
        .collect();
    email_list.sort();
    email_list.dedup();
    email_list
}

async fn get_todays_events() -> Vec<Event> {
    let local_time = Local::today().format("%Y-%m-%d").to_string();
    let events = get_like("calendar", "date", &local_time).await;
    events
        .iter()
        .map(|x| Event {
            id: None,
            title: from_value(x[1].clone()),
            date: None,
            start_time: from_value(x[3].clone()),
            end_time: from_value(x[4].clone()),
            practice: from_value(x[5].clone()),
        })
        .collect()
}

async fn send_reminder_email(
    reminder_list: Vec<String>,
    access_token: &str,
    todays_events: Vec<Event>,
) {
    if !todays_events.is_empty() {
        let mut email =
            "Hi,\r\nI just wanted to remind you that today we will be having our ".to_string();
        for i in 0..todays_events.len() {
            let event = format!(
                "{} from {} to {}",
                todays_events[i].title,
                todays_events[i].start_time.format(""),
                todays_events[i].end_time.format("")
            );
            if todays_events.len() == 1 {
                email = format!("{}{}.", email, event);
            } else if i == todays_events.len() - 1 {
                email = format!("{}, and our {}.", email, event);
            } else if i == 0 {
                email = format!("{}{}", email, event);
            } else {
                email = format!("{}, our {}", email, event);
            }
        }
        email = format!("{}\r\nThank you and God Bless!\r\n\r\nJustus", email);
        gmail::send_email(
            reminder_list,
            "Today's Children's Choir Events",
            &email,
            access_token,
        )
        .await;
    }
}

async fn get_calendar_events() -> Vec<Event> {
    let events = get_like("calendar", "send_email", "1").await;
    let events: Vec<Event> = events
        .iter()
        .map(|x| Event {
            id: from_value(x[0].clone()),
            title: from_value(x[1].clone()),
            date: from_value(x[2].clone()),
            start_time: from_value(x[3].clone()),
            end_time: from_value(x[4].clone()),
            practice: from_value(x[5].clone()),
        })
        .collect();
    for event in events.iter() {
        change_row_where(
            "calendar",
            "id",
            &event.id.unwrap().to_string(),
            "send_email",
            "0",
        )
        .await;
    }
    events
}

async fn emaild() -> Option<()> {
    let (access_token, title, email_list, events, todays_events, reminder_list) = futures::join!(
        get_access_token(),
        get_articles(),
        get_email_list(),
        get_calendar_events(),
        get_todays_events(),
        get_reminder_list(),
    );
    let access_token = access_token?;
    futures::join!(
        send_article_email(email_list.clone(), &access_token, title),
        send_calendar_email(email_list, &access_token, events),
        send_reminder_email(reminder_list, &access_token, todays_events),
    );
    Some(())
}

#[tokio::main]
async fn main() {
    if let None = emaild().await {
        println!("Error");
    }
}
