use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use rocket::fairing::AdHoc;
use surrealdb::engine::remote::ws::Client;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use surrealdb::opt::Resource;
use surrealdb::sql::{Id, Thing};
use surrealdb::Surreal;

use crate::EventStatus;
use crate::{Audit, AuditType, Counter, Event, Group, Membership, MyConfig, User, RSVP};

/// # Panics
///
/// Panics when it fails to create the database folder or set up the database.
#[must_use]
pub fn fairing() -> AdHoc {
    // TODO handle errors here properly by using AdHoc::try_on_ignite instead of AdHoc::on_ignite.
    AdHoc::on_ignite("Managed Database Connection", |rocket| async {
        let config = rocket.state::<MyConfig>().unwrap();

        let dbh = get_database(
            &config.database_username,
            &config.database_password,
            &config.database_name,
            &config.database_namespace,
        )
        .await;

        rocket.manage(dbh)
    })
}

/// # Panics
///
/// Panics when it fails to create the database folder or set up the database.
pub async fn get_database(
    username: &str,
    password: &str,
    db_name: &str,
    db_namespace: &str,
) -> Surreal<Client> {
    let address = "127.0.0.1:8000";
    let dbh = Surreal::new::<Ws>(address).await.unwrap();

    dbh.signin(Root { username, password }).await.unwrap();

    dbh.use_ns(db_namespace).use_db(db_name).await.unwrap();

    upgrade(&dbh).await.unwrap();

    dbh
}

/// # Panics
///
/// Panics when there is an error
pub async fn upgrade(dbh: &Surreal<Client>) -> surrealdb::Result<()> {
    let version = get_schema_version(dbh).await.unwrap();
    rocket::info!("Upgrade from {version}");

    if version == 0 {
        upgrade_to_1(dbh).await?;
        upgrade_to_2(dbh).await?;
        upgrade_to_3(dbh).await?;
    }
    if version == 1 {
        upgrade_to_2(dbh).await?;
    }
    if version == 2 {
        upgrade_to_3(dbh).await?;
    }

    Ok(())
}

/// # Panics
///
/// Panics when there is an error
pub async fn upgrade_to_1(dbh: &Surreal<Client>) -> surrealdb::Result<()> {
    rocket::info!("upgrade_to_1");

    dbh.query("DEFINE INDEX user_email ON TABLE user COLUMNS email UNIQUE")
        .await
        .unwrap();

    dbh.query("DEFINE INDEX user_uid ON TABLE user COLUMNS uid UNIQUE")
        .await
        .unwrap();

    dbh.query("DEFINE INDEX group_gid ON TABLE group COLUMNS gid UNIQUE")
        .await
        .unwrap();

    dbh.query("DEFINE INDEX member_ship ON TABLE membership COLUMNS uid, gid UNIQUE")
        .await
        .unwrap();

    dbh.query("DEFINE INDEX rsvp_index ON TABLE rsvp COLUMNS uid, eid UNIQUE")
        .await
        .unwrap();

    create_schema_version(dbh).await?;
    Ok(())
}

/// # Panics
///
/// Panics when there is an error
pub async fn upgrade_to_2(dbh: &Surreal<Client>) -> surrealdb::Result<()> {
    rocket::info!("upgrade_to_2");

    dbh.query("UPDATE event SET status=$status")
        .bind(("status", EventStatus::Published))
        .await?;

    update_schema_version(dbh, 2).await?;
    Ok(())
}

/// # Panics
///
/// Panics when there is an error
pub async fn upgrade_to_3(dbh: &Surreal<Client>) -> surrealdb::Result<()> {
    rocket::info!("upgrade_to_3");

    dbh.query("DELETE audit").await?;

    update_schema_version(dbh, 3).await?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Schema {
    version: u64,
}

async fn get_schema_version(dbh: &Surreal<Client>) -> surrealdb::Result<u64> {
    let mut response = dbh.query("SELECT * from schema").await?;
    let versions: Vec<Schema> = response.take(0)?;
    if let Some(schema) = versions.first() {
        return Ok(schema.version);
    }

    Ok(0)
}

async fn create_schema_version(dbh: &Surreal<Client>) -> surrealdb::Result<()> {
    let _created: Option<Schema> = dbh
        .create("schema")
        .content(Schema { version: 1 })
        .await
        .unwrap();

    Ok(())
}

async fn update_schema_version(dbh: &Surreal<Client>, version: u64) -> surrealdb::Result<()> {
    dbh.query("UPDATE schema SET version=$version")
        .bind(("version", version))
        .await?;

    Ok(())
}

pub async fn add_user(dbh: &Surreal<Client>, user: &User) -> surrealdb::Result<()> {
    rocket::info!("add user email: '{}'", user.email);

    dbh.create(Resource::from("user"))
        .content(user.clone())
        .await?;

    Ok(())
}

pub async fn add_event(dbh: &Surreal<Client>, event: &Event) -> surrealdb::Result<()> {
    rocket::info!("add event eid: '{}' title: '{}'", event.eid, event.title);

    dbh.create(Resource::from("event"))
        .content(event.clone())
        .await?;

    Ok(())
}

pub async fn update_event(dbh: &Surreal<Client>, event: &Event) -> surrealdb::Result<()> {
    rocket::info!(
        "update_event: eid: '{}' new title: '{}'",
        event.eid,
        event.title
    );

    let mut response = dbh
        .query(
            "
            UPDATE event
                SET
                    title=$title,
                    date=$date,
                    location=$location,
                    description=$description
                WHERE eid=$eid;",
        )
        .bind(("title", event.title.clone()))
        .bind(("location", event.location.clone()))
        .bind(("date", event.date))
        .bind(("description", event.description.clone()))
        .bind(("eid", event.eid))
        .await?;

    rocket::info!("response {:?}", response);
    let entry: Option<Event> = response.take(0)?;
    if let Some(entry) = entry.as_ref() {
        rocket::info!("event updated '{}', '{}'", entry.title, entry.date);
    }

    Ok(())
}

pub async fn add_group(dbh: &Surreal<Client>, group: &Group) -> surrealdb::Result<()> {
    rocket::info!("add group: '{}'", group.name);

    dbh.create(Resource::from("group"))
        .content(group.clone())
        .await?;

    Ok(())
}

pub async fn set_user_verified(
    dbh: &Surreal<Client>,
    uid: usize,
) -> surrealdb::Result<Option<User>> {
    rocket::info!("set_user_verified: '{uid}'");
    let utc: DateTime<Utc> = Utc::now();
    let mut response = dbh
        .query(
            "
            UPDATE user
                SET
                    verified=$verified,
                    code='',
                    verification_date=$date
                WHERE uid=$uid;",
        )
        .bind(("verified", true))
        .bind(("date", utc))
        .bind(("uid", uid))
        .await?;

    let entry: Option<User> = response.take(0)?;

    if let Some(entry) = entry.as_ref() {
        rocket::info!(
            "verification ok '{}', '{}', '{}'",
            entry.name,
            entry.email,
            entry.process
        );
    }

    Ok(entry)
}

pub async fn update_group(
    dbh: &Surreal<Client>,
    gid: usize,
    name: &str,
    location: &str,
    description: &str,
) -> surrealdb::Result<Option<Group>> {
    rocket::info!("update group: '{gid}'");

    let mut response = dbh
        .query(
            "
            UPDATE group
            SET
                name=$name,
                location=$location,
                description=$description
            WHERE gid=$gid;",
        )
        .bind(("name", name.to_owned()))
        .bind(("location", location.to_owned()))
        .bind(("description", description.to_owned()))
        .bind(("gid", gid))
        .await?;

    let entry: Option<Group> = response.take(0)?;
    Ok(entry)
}

pub async fn remove_code(dbh: &Surreal<Client>, uid: usize) -> surrealdb::Result<Option<User>> {
    rocket::info!("remove code '{uid}'");

    let mut response = dbh
        .query(
            "
            UPDATE user
            SET
                code='',
                code_generated_date=None
            WHERE uid=$uid;",
        )
        .bind(("uid", uid))
        .await?;

    let entry: Option<User> = response.take(0)?;
    Ok(entry)
}

pub async fn save_password(
    dbh: &Surreal<Client>,
    uid: usize,
    password: &str,
) -> surrealdb::Result<Option<User>> {
    rocket::info!("save password for '{uid}'");

    let mut response = dbh
        .query(
            "
            UPDATE user
            SET
                password=$password
            WHERE uid=$uid;",
        )
        .bind(("password", password.to_owned()))
        .bind(("uid", uid))
        .await?;

    let entry: Option<User> = response.take(0)?;
    Ok(entry)
}

pub async fn update_user(
    dbh: &Surreal<Client>,
    uid: usize,
    name: &str,
    github: &str,
    gitlab: &str,
    linkedin: &str,
    about: &str,
) -> surrealdb::Result<Option<User>> {
    rocket::info!("update user: '{uid}'");

    let mut response = dbh
        .query(
            "
            UPDATE user
            SET
                name=$name,
                about=$about,
                gitlab=$gitlab,
                linkedin=$linkedin,
                github=$github
            WHERE uid=$uid;",
        )
        .bind(("name", name.to_owned()))
        .bind(("github", github.to_owned()))
        .bind(("gitlab", gitlab.to_owned()))
        .bind(("linkedin", linkedin.to_owned()))
        .bind(("uid", uid))
        .bind(("about", about.to_owned()))
        .await?;

    let entry: Option<User> = response.take(0)?;
    Ok(entry)
}

pub async fn get_user_by_uid(dbh: &Surreal<Client>, uid: usize) -> surrealdb::Result<Option<User>> {
    rocket::info!("get_user_by_uid: '{uid}'");

    let mut response = dbh
        .query("SELECT * FROM user WHERE uid=$uid;")
        .bind(("uid", uid))
        .await?;

    let entry: Option<User> = response.take(0)?;

    if let Some(entry) = entry.as_ref() {
        rocket::info!("Found user {}, {}", entry.name, entry.email);
    }

    Ok(entry)
}

pub async fn get_user_by_id(dbh: &Surreal<Client>, id: Id) -> surrealdb::Result<Option<User>> {
    rocket::info!("get_user_by_id: '{id}'");

    let mut response = dbh
        .query("SELECT * FROM user WHERE id=$id;")
        .bind(("id", Thing::from(("user", id))))
        .await?;

    let entry: Option<User> = response.take(0)?;

    Ok(entry)
}

pub async fn get_user_by_email(
    dbh: &Surreal<Client>,
    email: &str,
) -> surrealdb::Result<Option<User>> {
    rocket::info!("get_user_by_email: '{email}'");
    let mut response = dbh
        .query("SELECT * FROM user WHERE email=$email;")
        .bind(("email", email.to_owned()))
        .await?;

    let entry: Option<User> = response.take(0)?;

    if let Some(entry) = entry.as_ref() {
        rocket::info!("************* {}, {}", entry.name, entry.email);
    }

    Ok(entry)
}

pub async fn add_login_code_to_user(
    dbh: &Surreal<Client>,
    email: &str,
    process: &str,
    code: &str,
) -> surrealdb::Result<Option<User>> {
    rocket::info!("add_login_code_to_user: '{email}', '{process}', '{code}'");
    let utc: DateTime<Utc> = Utc::now();
    let mut response = dbh
    .query("UPDATE user SET code=$code, process=$process, code_generated_date=$date WHERE email=$email;")
        .bind(("email", email.to_owned()))
        .bind(("process", process.to_owned()))
        .bind(("code", code.to_owned()))
        .bind(("date", utc))
        .await?;

    let entry: Option<User> = response.take(0)?;

    if let Some(entry) = entry.as_ref() {
        rocket::info!("entry: '{}' '{}'", entry.email, entry.process);
    }

    Ok(entry)
}

#[must_use]
pub async fn get_events_by_group_id(dbh: &Surreal<Client>, gid: usize) -> Vec<Event> {
    match get_events(dbh).await {
        Ok(events) => events
            .into_iter()
            .filter(|event| event.group_id == gid)
            .collect(),
        Err(_) => vec![],
    }
}

pub async fn get_users(dbh: &Surreal<Client>) -> surrealdb::Result<Vec<User>> {
    rocket::info!("get_users");
    let mut response = dbh.query("SELECT * FROM user;").await?;
    let entries: Vec<User> = response.take(0)?;
    for ent in &entries {
        rocket::info!("user name {}", ent.name);
    }
    Ok(entries)
}

pub async fn get_groups(dbh: &Surreal<Client>) -> surrealdb::Result<Vec<Group>> {
    rocket::info!("get_groups");
    let mut response = dbh.query("SELECT * FROM group ORDER BY name;").await?;
    let entries: Vec<Group> = response.take(0)?;
    for ent in &entries {
        rocket::info!("group name {}", ent.name);
    }
    Ok(entries)
}

/// # Panics
///
/// Panics when there is an error
pub async fn get_groups_by_membership_id(
    dbh: &Surreal<Client>,
    uid: usize,
) -> surrealdb::Result<Vec<(Group, Membership)>> {
    rocket::info!("get_groups_by_membership_id: '{uid}'");

    // let mut response = dbh
    // .query("SELECT * FROM membership WHERE uid=$uid;")
    // .bind(("uid", uid))
    // .await?;

    // let entries: Vec<Membership> = response.take(0)?;
    // rocket::info!("gids: {entries:?}");

    // let mut response = dbh
    // .query("SELECT gid FROM membership WHERE uid=$uid;")
    // .bind(("uid", uid))
    // .await?;

    // let entries: Vec<String> = response.take(0)?;
    // rocket::info!("gids: {entries:?}");

    // let mut response = dbh
    //     .query("SELECT * FROM group WHERE gid IN (SELECT gid FROM membership WHERE uid=$uid);")
    //     .bind(("uid", uid))
    //     .await?;

    // let mut response = dbh
    //     .query("SELECT * FROM group, membership WHERE group.gid=membership.gid AND membership.uid=$uid;")
    //     .bind(("uid", uid))
    //     .await?;

    // TODO: make the code above with subexpression work
    let mut response = dbh
        .query("SELECT * FROM membership WHERE uid=$uid;")
        .bind(("uid", uid))
        .await?;

    let memberships: Vec<Membership> = response.take(0)?;
    rocket::info!("members: {memberships:?}");

    let mut groups = vec![];
    for member in memberships {
        rocket::info!("gid: {}", member.gid);
        let mut response2 = dbh
            .query("SELECT * FROM group WHERE gid=$gid;")
            .bind(("gid", member.gid))
            .await?;

        let entries: Vec<Group> = response2.take(0)?;
        rocket::info!("entries: {entries:?}");
        let group = entries.first().unwrap().clone();
        //groups.push((group, member.join_date));
        groups.push((group, member));
    }

    Ok(groups)
}

/// # Panics
///
/// Panics when there is an error
pub async fn get_members_of_group(
    dbh: &Surreal<Client>,
    gid: usize,
) -> surrealdb::Result<Vec<(User, Membership)>> {
    rocket::info!("get_members_of_group: '{gid}'");

    let mut response = dbh
        .query("SELECT * FROM membership WHERE gid=$gid;")
        .bind(("gid", gid))
        .await?;

    let memberships: Vec<Membership> = response.take(0)?;
    rocket::info!("members: {memberships:?}");

    let mut users = vec![];
    for member in memberships {
        rocket::info!("uid: {}", member.uid);
        let mut response2 = dbh
            .query("SELECT * FROM user WHERE uid=$uid;")
            .bind(("uid", member.uid))
            .await?;

        let entries: Vec<User> = response2.take(0)?;
        rocket::info!("entries: {entries:?}");
        let user = entries.first().unwrap().clone();
        users.push((user, member));
    }

    Ok(users)
}

pub async fn get_groups_by_owner_id(
    dbh: &Surreal<Client>,
    uid: usize,
) -> surrealdb::Result<Vec<Group>> {
    rocket::info!("get_groups_by_owner_id: '{uid}'");
    let mut response = dbh
        .query("SELECT * FROM group WHERE owner=$uid ORDER BY name;")
        .bind(("uid", uid))
        .await?;

    let entries: Vec<Group> = response.take(0)?;

    Ok(entries)
}

pub async fn get_group_by_gid(
    dbh: &Surreal<Client>,
    gid: usize,
) -> surrealdb::Result<Option<Group>> {
    rocket::info!("get_group_by_gid: '{gid}'");
    let mut response = dbh
        .query("SELECT * FROM group WHERE gid=$gid;")
        .bind(("gid", gid))
        .await?;

    let entry: Option<Group> = response.take(0)?;

    if let Some(entry) = entry.as_ref() {
        rocket::info!("Group name: {}", entry.name);
    }

    Ok(entry)
}

pub async fn get_events(dbh: &Surreal<Client>) -> surrealdb::Result<Vec<Event>> {
    rocket::info!("get_events");
    let mut response = dbh.query("SELECT * FROM event;").await?;
    let entries: Vec<Event> = response.take(0)?;
    for ent in &entries {
        rocket::info!("event name {}", ent.title);
    }
    Ok(entries)
}

/// # Panics
///
/// Panics when there is an error
pub async fn increment(dbh: &Surreal<Client>, name: &str) -> surrealdb::Result<usize> {
    // TODO: do this only when creatig the database
    let _response = dbh
        .query("DEFINE INDEX counter_name ON TABLE counter COLUMNS name UNIQUE")
        .await?;

    let response = dbh
        .query(
            "
            INSERT INTO counter (name, count)
                VALUES ($name, $count) ON DUPLICATE KEY UPDATE count += 1;
        ",
        )
        .bind(("name", name.to_owned()))
        .bind(("count", 1_i32))
        .await?;

    let mut entries = response.check()?;
    let entries: Vec<Counter> = entries.take(0)?;
    // fetching the first (and hopefully only) entry
    let entry = entries.into_iter().next().unwrap();
    let id: usize = entry.count.try_into().unwrap();

    Ok(id)
}

pub async fn get_event_by_eid(
    dbh: &Surreal<Client>,
    eid: usize,
) -> surrealdb::Result<Option<Event>> {
    rocket::info!("get_event_by_eid: '{eid}'");
    let mut response = dbh
        .query("SELECT * FROM event WHERE eid=$eid;")
        .bind(("eid", eid))
        .await?;

    let entry: Option<Event> = response.take(0)?;

    if let Some(entry) = entry.as_ref() {
        rocket::info!("Event title: {}", entry.title);
    }

    Ok(entry)
}

pub async fn join_group(dbh: &Surreal<Client>, gid: usize, uid: usize) -> surrealdb::Result<()> {
    rocket::info!("user {} joins group: {}", uid, gid);

    let date: DateTime<Utc> = Utc::now();

    let membership = Membership {
        id: Thing::from(("membership", Id::ulid())),
        uid,
        gid,
        join_date: date,
        admin: false,
    };

    dbh.create(Resource::from("membership"))
        .content(membership)
        .await?;

    Ok(())
}

/// # Panics
///
/// Panics when it fails
pub async fn leave_group(dbh: &Surreal<Client>, gid: usize, uid: usize) -> surrealdb::Result<()> {
    rocket::info!("user {} leaves group: {}", uid, gid);

    dbh.query("DELETE membership WHERE uid=$uid AND gid=$gid")
        .bind(("uid", uid))
        .bind(("gid", gid))
        .await?
        .check()
        .unwrap();

    Ok(())
}

pub async fn get_membership(
    dbh: &Surreal<Client>,
    gid: usize,
    uid: usize,
) -> surrealdb::Result<Option<Membership>> {
    let mut response = dbh
        .query("SELECT * FROM membership WHERE gid=$gid AND uid=$uid;")
        .bind(("gid", gid))
        .bind(("uid", uid))
        .await?;

    let entry: Option<Membership> = response.take(0)?;

    Ok(entry)
}

/// # Panics
///
/// Panics when there is an error.
pub async fn get_all_rsvps_for_event(
    dbh: &Surreal<Client>,
    eid: usize,
) -> surrealdb::Result<Vec<(RSVP, User)>> {
    let mut response = dbh
        .query("SELECT * FROM rsvp WHERE eid=$eid ORDER BY uid;")
        .bind(("eid", eid))
        .await?;

    let entries: Vec<RSVP> = response.take(0)?;

    let mut people = vec![];
    for entry in entries {
        // We assume that each uid will have a user
        let user = get_user_by_uid(dbh, entry.uid).await.unwrap().unwrap();
        people.push((entry, user));
    }

    Ok(people)
}

pub async fn get_rsvp(
    dbh: &Surreal<Client>,
    eid: usize,
    uid: usize,
) -> surrealdb::Result<Option<RSVP>> {
    let mut response = dbh
        .query("SELECT * FROM rsvp WHERE eid=$eid AND uid=$uid;")
        .bind(("eid", eid))
        .bind(("uid", uid))
        .await?;

    let entry: Option<RSVP> = response.take(0)?;

    Ok(entry)
}

pub async fn new_rsvp(
    dbh: &Surreal<Client>,
    eid: usize,
    uid: usize,
    status: bool,
) -> surrealdb::Result<()> {
    rocket::info!("user {} RSVP: {} status: {}", uid, eid, status);

    let date: DateTime<Utc> = Utc::now();

    let rsvp = RSVP {
        id: Thing::from(("rsvp", Id::ulid())),
        eid,
        uid,
        date,
        status,
    };

    dbh.create(Resource::from("rsvp")).content(rsvp).await?;

    Ok(())
}

pub async fn update_rsvp(
    dbh: &Surreal<Client>,
    eid: usize,
    uid: usize,
    status: bool,
) -> surrealdb::Result<()> {
    rocket::info!("user {} RSVP: {} status: {}", uid, eid, status);

    let date: DateTime<Utc> = Utc::now();

    dbh.query("UPDATE rsvp SET status=$status, date=$date WHERE uid=$uid AND eid=$eid")
        .bind(("status", status))
        .bind(("uid", uid))
        .bind(("eid", eid))
        .bind(("date", date))
        .await?;

    Ok(())
}

pub async fn audit(dbh: &Surreal<Client>, atype: AuditType, json: Value) -> surrealdb::Result<()> {
    let text = json.to_string();
    rocket::info!("audit {text}");

    let date: DateTime<Utc> = Utc::now();
    let entry = Audit {
        id: Thing::from(("audit", Id::ulid())),
        date,
        atype,
        text,
    };

    dbh.create(Resource::from("audit")).content(entry).await?;

    Ok(())
}

pub async fn get_audit(dbh: &Surreal<Client>) -> surrealdb::Result<Vec<Audit>> {
    rocket::info!("get_audits");
    let mut response = dbh.query("SELECT * FROM audit ORDER BY date;").await?;
    let entries: Vec<Audit> = response.take(0)?;
    Ok(entries)
}

pub async fn get_user_by_id_str(
    dbh: &Surreal<Client>,
    id: &str,
) -> surrealdb::Result<Option<User>> {
    rocket::info!("get_user_by_id: '{id}'");

    let mut response = dbh
        .query("SELECT * FROM user WHERE id=$id;")
        .bind(("id", Thing::from(("user", id))))
        .await?;

    let entry: Option<User> = response.take(0)?;

    Ok(entry)
}
