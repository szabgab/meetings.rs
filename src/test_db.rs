use chrono::{DateTime, Utc};
use serde_json::json;

use surrealdb::engine::remote::ws::Client;
use surrealdb::sql::{Id, Thing};
use surrealdb::Surreal;

use crate::db;
use meetings::{db::get_user_by_uid, AuditType, Event, EventStatus, Group, Membership, User, RSVP};

use crate::test_lib::{ADMIN_EMAIL, ADMIN_NAME, OWNER_EMAIL, OWNER_NAME, USER_EMAIL, USER_NAME};

async fn setup() -> (Surreal<Client>, String) {
    let db_namespace = String::from("test-namespace-for-meet-os");
    let db_name = format!("test-name-{}", rand::random::<f64>());

    let dbh = db::get_database("root", "root", &db_name, &db_namespace).await;

    (dbh, db_name)
}

async fn teardown(dbh: Surreal<Client>, db_name: String) {
    let _res = dbh
        .query("REMOVE DATABASE `$name`")
        .bind(("name", db_name))
        .await
        .unwrap();
}

async fn add_admin_helper(dbh: &Surreal<Client>) {
    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let user = User {
        id: Thing::from(("user", id)),
        uid: 1,
        name: ADMIN_NAME.to_owned(),
        email: ADMIN_EMAIL.to_owned(),
        password: String::from("should be hashed password"),
        code: String::from("generated code"),
        process: String::from("register"),
        verified: false,
        registration_date: utc,
        verification_date: None,
        code_generated_date: Some(utc),
        github: None,
        gitlab: None,
        linkedin: None,
        about: None,
    };

    let res = db::add_user(&dbh, &user).await.unwrap();
    assert_eq!(res, ());
}

async fn add_owner_helper(dbh: &Surreal<Client>) {
    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let user = User {
        id: Thing::from(("user", id)),
        uid: 2,
        name: OWNER_NAME.to_owned(),
        email: OWNER_EMAIL.to_owned(),
        password: String::from("should be hashed password"),
        code: String::from("generated code"),
        process: String::from("register"),
        verified: false,
        registration_date: utc,
        verification_date: None,
        code_generated_date: Some(utc),

        github: None,
        gitlab: None,
        linkedin: None,
        about: None,
    };

    let res = db::add_user(&dbh, &user).await.unwrap();
    assert_eq!(res, ());
}

async fn add_user_helper(dbh: &Surreal<Client>) {
    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let user = User {
        id: Thing::from(("user", id)),
        uid: 3,
        name: USER_NAME.to_owned(),
        email: USER_EMAIL.to_owned(),
        password: String::from("should be hashed password"),
        code: String::from("generated code"),
        process: String::from("register"),
        verified: false,
        registration_date: utc,
        verification_date: None,
        code_generated_date: Some(utc),

        github: None,
        gitlab: None,
        linkedin: None,
        about: None,
    };

    let res = db::add_user(&dbh, &user).await.unwrap();
    assert_eq!(res, ());
}

async fn add_groups_helper(dbh: &Surreal<Client>) {
    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let rust_maven = Group {
        id: Thing::from(("group", id)),
        gid: 1,
        owner: 2,
        name: String::from("Rust Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &rust_maven).await.unwrap();
    assert_eq!(res, ());

    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let python_maven = Group {
        id: Thing::from(("group", id)),
        gid: 2,
        owner: 2,
        name: String::from("Python Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &python_maven).await.unwrap();
    assert_eq!(res, ());

    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let guest_maven = Group {
        id: Thing::from(("group", id)),
        gid: 3,
        owner: 3,
        name: String::from("Guest Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &guest_maven).await.unwrap();
    assert_eq!(res, ());
}

async fn add_events_helper(dbh: &Surreal<Client>) {
    let eid = 1;
    let title = "First Conference";
    let description = "";
    let date: DateTime<Utc> = Utc::now();
    let location = "";
    let gid = 1;

    let event = Event {
        id: Thing::from(("event", Id::ulid())),
        eid,
        title: title.to_owned(),
        description: description.to_owned(),
        date,
        location: location.to_owned(),
        group_id: gid,
        status: EventStatus::Published,
    };

    db::add_event(&dbh, &event).await.unwrap();

    let date: DateTime<Utc> = Utc::now();
    let event = Event {
        id: Thing::from(("event", Id::ulid())),
        eid: 2,
        title: String::from("Second conf"),
        description: String::new(),
        date,
        location: location.to_owned(),
        group_id: gid,
        status: EventStatus::Published,
    };

    db::add_event(&dbh, &event).await.unwrap();
}

#[async_test]
async fn test_db_get_empty_lists() {
    let (dbh, db_name) = setup().await;

    let events = db::get_events(&dbh).await.unwrap();
    assert!(events.is_empty());

    let audits = db::get_audit(&dbh).await.unwrap();
    assert!(audits.is_empty());

    let groups = db::get_groups(&dbh).await.unwrap();
    assert!(groups.is_empty());

    let eid = 1;
    let rsvps = db::get_all_rsvps_for_event(&dbh, eid).await.unwrap();
    assert!(rsvps.is_empty());

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_get_none() {
    let (dbh, db_name) = setup().await;

    let event = db::get_event_by_eid(&dbh, 1).await.unwrap();
    assert!(event.is_none());

    let user = db::get_user_by_email(&dbh, "bad_email").await.unwrap();
    assert!(user.is_none());

    let user = db::get_user_by_uid(&dbh, 23).await.unwrap();
    assert!(user.is_none());

    let eid = 1;
    let uid = 3;
    let rsvp = db::get_rsvp(&dbh, eid, uid).await.unwrap();
    assert!(rsvp.is_none());

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_user() {
    let (dbh, db_name) = setup().await;

    let utc: DateTime<Utc> = Utc::now();
    let id1 = Id::ulid();
    let user_foo = User {
        id: Thing::from(("user", id1.clone())),
        uid: 1,
        name: String::from("Foo Bar"),
        email: String::from("foo@meet-os.com"),
        password: String::from("should be hashed password"),
        code: String::from("generated code"),
        process: String::from("register"),
        verified: false,
        registration_date: utc,
        verification_date: None,
        code_generated_date: Some(utc),
        github: None,
        gitlab: None,
        linkedin: None,
        about: None,
    };

    let res = db::add_user(&dbh, &user_foo).await.unwrap();
    assert_eq!(res, ());

    let users = db::get_users(&dbh).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].name, user_foo.name);
    assert_eq!(users[0], user_foo);

    let id2 = Id::ulid();
    let other_user = User {
        id: Thing::from(("user", id2.clone())),
        code: String::from("other code"),
        uid: 2,
        ..user_foo.clone()
    };
    let res = db::add_user(&dbh, &other_user).await;
    assert!(res.is_err());
    let err = res.err().unwrap().to_string();
    //println!("{err}");
    assert!(err.contains("There was a problem with the database: Database index `user_email` already contains 'foo@meet-os.com'"));

    let id3 = Id::ulid();
    let other_user = User {
        id: Thing::from(("user", id3.clone())),
        code: String::from("other code"),
        email: String::from("peti@meet-os.com"),
        ..user_foo.clone()
    };

    let res = db::add_user(&dbh, &other_user).await;
    assert!(res.is_err());
    let err = res.err().unwrap().to_string();
    assert!(err.contains(
        "There was a problem with the database: Database index `user_uid` already contains 1"
    ));

    println!("id1: {id1}, id2: {id2}, id3: {id3}");
    let user1 = db::get_user_by_id(&dbh, id1).await.unwrap().unwrap();
    assert_eq!(user1.email, "foo@meet-os.com");

    // TODO make sure we don't accidentally use the same code twice
    // let other_user = User {
    //     uid: 2,
    //     email: String::from("peti@meet-os.com"),
    //     ..user_foo.clone()
    // };

    // let res = db::add_user(&dbh, &other_user).await;
    // assert!(res.is_err(), "Should not be able to use the same code twice");
    // let err = res.err().unwrap().to_string();
    // //assert_eq!(err, "");
    // assert!(err.contains(
    //     "There was a problem with the database: Database index `user_code` already contains 'generated code'"
    // ));

    let id = Id::ulid();
    let user_peti = User {
        id: Thing::from(("user", id)),
        uid: 2,
        name: String::from("Peti Bar"),
        email: String::from("peti@meet-os.com"),
        code: String::from("some other code"),
        ..user_foo.clone()
    };
    let res = db::add_user(&dbh, &user_peti).await.unwrap();
    assert_eq!(res, ());

    let users = db::get_users(&dbh).await.unwrap();
    assert_eq!(users.len(), 2);
    // TODO: should we fix the order? Without that these test need to take into account the lack of order
    // assert_eq!(users[0], user_foo);
    // assert_eq!(users[1], user_peti);

    let user = db::get_user_by_email(&dbh, "foo@meet-os.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user, user_foo);

    let user = db::get_user_by_uid(&dbh, 1).await.unwrap().unwrap();
    assert_eq!(user, user_foo);

    // Add group
    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let rust_maven = Group {
        id: Thing::from(("group", id)),
        gid: 1,
        owner: 2,
        name: String::from("Rust Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &rust_maven).await.unwrap();
    assert_eq!(res, ());

    let groups = db::get_groups(&dbh).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0], rust_maven);

    // Try to add another group with the same gid
    let id = Id::ulid();
    let other_group = Group {
        id: Thing::from(("group", id)),
        ..rust_maven.clone()
    };
    let res = db::add_group(&dbh, &other_group).await;
    assert!(res.is_err(), "Should not be able to use the same gid twice");
    let err = res.err().unwrap().to_string();
    assert!(err.contains(
        "There was a problem with the database: Database index `group_gid` already contains 1"
    ));

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_groups() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_user_helper(&dbh).await;

    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let rust_maven = Group {
        id: Thing::from(("group", id)),
        gid: 1,
        owner: 2,
        name: String::from("Rust Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &rust_maven).await.unwrap();
    assert_eq!(res, ());

    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let python_maven = Group {
        id: Thing::from(("group", id)),
        gid: 2,
        owner: 2,
        name: String::from("Python Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &python_maven).await.unwrap();
    assert_eq!(res, ());

    let utc: DateTime<Utc> = Utc::now();
    let id = Id::ulid();
    let guest_maven = Group {
        id: Thing::from(("group", id)),
        gid: 3,
        owner: 3,
        name: String::from("Guest Maven"),
        location: String::new(),
        description: String::new(),
        creation_date: utc,
    };
    let res = db::add_group(&dbh, &guest_maven).await.unwrap();
    assert_eq!(res, ());

    let groups = db::get_groups(&dbh).await.unwrap();
    assert_eq!(groups.len(), 3);
    assert_eq!(
        groups,
        [
            guest_maven.clone(),
            python_maven.clone(),
            rust_maven.clone()
        ]
    );

    let groups = db::get_groups_by_owner_id(&dbh, 1).await.unwrap();
    assert_eq!(groups, []);

    let groups = db::get_groups_by_owner_id(&dbh, 2).await.unwrap();
    assert_eq!(groups, [python_maven.clone(), rust_maven.clone()]);

    let groups = db::get_groups_by_owner_id(&dbh, 3).await.unwrap();
    assert_eq!(groups, [guest_maven.clone()]);

    let group = db::get_group_by_gid(&dbh, 1).await.unwrap().unwrap();
    assert_eq!(group, rust_maven);

    let group = db::get_group_by_gid(&dbh, 2).await.unwrap().unwrap();
    assert_eq!(group, python_maven);

    let group = db::get_group_by_gid(&dbh, 3).await.unwrap().unwrap();
    assert_eq!(group, guest_maven);

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_group_membership() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_user_helper(&dbh).await;
    add_groups_helper(&dbh).await;

    let gid = 1;
    let members = db::get_members_of_group(&dbh, gid).await.unwrap();
    assert!(members.is_empty());

    let uid = 1;
    let membership = db::get_membership(&dbh, gid, uid).await.unwrap();
    assert!(membership.is_none());

    db::join_group(&dbh, gid, uid).await.unwrap();
    let membership = db::get_membership(&dbh, gid, uid).await.unwrap().unwrap();
    //println!("{membership:?}");
    assert_eq!(
        membership,
        Membership {
            id: membership.id.clone(),
            gid: 1,
            uid: 1,
            join_date: membership.join_date,
            admin: false
        }
    );

    let uid = 2;
    db::join_group(&dbh, gid, uid).await.unwrap();
    let membership = db::get_membership(&dbh, gid, uid).await.unwrap().unwrap();
    //println!("{membership:?}");
    assert_eq!(
        membership,
        Membership {
            id: membership.id.clone(),
            gid: 1,
            uid: 2,
            join_date: membership.join_date,
            admin: false
        }
    );

    let group_membership = db::get_groups_by_membership_id(&dbh, uid).await.unwrap();
    println!("{group_membership:?}");

    let members = db::get_members_of_group(&dbh, gid).await.unwrap();
    assert_eq!(members.len(), 2);

    db::leave_group(&dbh, gid, uid).await.unwrap();

    let members = db::get_members_of_group(&dbh, gid).await.unwrap();
    assert_eq!(members.len(), 1);

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_events() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_user_helper(&dbh).await;
    add_groups_helper(&dbh).await;

    let eid = db::increment(&dbh, "event").await.unwrap();

    let title = "First Conference";
    let description = "";
    let date: DateTime<Utc> = Utc::now();
    let location = "";
    let gid = 1;

    let event = Event {
        id: Thing::from(("event", Id::ulid())),
        eid,
        title: title.to_owned(),
        description: description.to_owned(),
        date,
        location: location.to_owned(),
        group_id: gid,
        status: EventStatus::Published,
    };

    db::add_event(&dbh, &event).await.unwrap();

    let events = db::get_events(&dbh).await.unwrap();
    // println!("{:#?}", events);
    assert_eq!(events.len(), 1);

    assert_eq!(events, [event.clone()]);

    let this_event = db::get_event_by_eid(&dbh, 1).await.unwrap().unwrap();
    assert_eq!(this_event, event);

    let group_events = db::get_events_by_group_id(&dbh, 1).await;
    assert_eq!(group_events, [event.clone()]);

    let group_events = db::get_events_by_group_id(&dbh, 2).await;
    assert!(group_events.is_empty());

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_rsvp() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_user_helper(&dbh).await;
    add_groups_helper(&dbh).await;
    add_events_helper(&dbh).await;

    let eid = 1;
    let uid = 1;
    db::new_rsvp(&dbh, eid, uid, true).await.unwrap();

    let uid = 2;
    db::new_rsvp(&dbh, eid, uid, true).await.unwrap();

    let rsvps = db::get_all_rsvps_for_event(&dbh, eid).await.unwrap();
    assert_eq!(rsvps.len(), 2);
    //println!("{rsvps:?}");
    assert_eq!(rsvps[0].0.eid, 1);
    assert_eq!(rsvps[0].1.uid, 1);

    assert_eq!(rsvps[1].0.eid, 1);
    assert_eq!(rsvps[1].1.uid, 2);

    let rsvp = db::get_rsvp(&dbh, eid, uid).await.unwrap().unwrap();
    assert_eq!(
        rsvp,
        RSVP {
            id: rsvp.id.clone(),
            eid: 1,
            uid: 2,
            status: true,
            date: rsvp.date
        }
    );

    let rsvp = db::get_rsvp(&dbh, eid, 1).await.unwrap().unwrap();
    assert_eq!(
        rsvp,
        RSVP {
            id: rsvp.id.clone(),
            eid: 1,
            uid: 1,
            status: true,
            date: rsvp.date
        }
    );

    db::update_rsvp(&dbh, eid, uid, false).await.unwrap();
    let rsvp = db::get_rsvp(&dbh, eid, uid).await.unwrap().unwrap();
    assert_eq!(
        rsvp,
        RSVP {
            id: rsvp.id.clone(),
            eid: 1,
            uid: 2,
            status: false,
            date: rsvp.date
        }
    );

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_increment() {
    let (dbh, db_name) = setup().await;

    let people = db::increment(&dbh, "people").await.unwrap();
    assert_eq!(people, 1);

    let people = db::increment(&dbh, "people").await.unwrap();
    assert_eq!(people, 2);

    let cars = db::increment(&dbh, "cars").await.unwrap();
    assert_eq!(cars, 1);

    let people = db::increment(&dbh, "people").await.unwrap();
    assert_eq!(people, 3);

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_audit() {
    let (dbh, db_name) = setup().await;

    db::audit(
        &dbh,
        AuditType::GroupCreated,
        json!({
            "owner": {
                "uid": 1,
                "name": "Owner"
            }
        }),
    )
    .await
    .unwrap();

    db::audit(
        &dbh,
        AuditType::JoinGroup,
        json!({
            "user": {
                "uid": 1,
                "name": "Foo Bar",
            }
        }),
    )
    .await
    .unwrap();

    db::audit(
        &dbh,
        AuditType::RSVPYes,
        json!({
            "user": {
                "uid": 2,
                "name": "Active Person",
            }
        }),
    )
    .await
    .unwrap();

    let audit = db::get_audit(&dbh).await.unwrap();
    println!("{audit:?}");
    assert_eq!(audit.len(), 3);
    assert_eq!(audit[0].atype, AuditType::GroupCreated);
    assert_eq!(audit[1].atype, AuditType::JoinGroup);
    assert_eq!(audit[2].atype, AuditType::RSVPYes);
    // assert_eq!(audit[0].text, "First message");
    // assert_eq!(audit[1].text, "Second message");
    // assert_eq!(audit[2].text, "And one more");

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_code() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_user_helper(&dbh).await;

    let user = db::get_user_by_uid(&dbh, 3).await.unwrap().unwrap();
    assert_eq!(user.name, USER_NAME);
    assert_eq!(user.code, "generated code");
    assert_eq!(user.verified, false);
    assert!(user.verification_date.is_none());

    db::set_user_verified(&dbh, 3).await.unwrap();

    let user = db::get_user_by_uid(&dbh, 3).await.unwrap().unwrap();
    assert_eq!(user.name, USER_NAME);
    assert_eq!(user.code, "");
    assert_eq!(user.verified, true);
    assert!(user.verification_date.is_some());

    let user = db::get_user_by_uid(&dbh, 2).await.unwrap().unwrap();
    assert_eq!(user.name, OWNER_NAME);
    assert_eq!(user.code, "generated code");
    assert_eq!(user.verified, false);
    assert!(user.verification_date.is_none());

    db::remove_code(&dbh, 2).await.unwrap();
    let user = db::get_user_by_uid(&dbh, 2).await.unwrap().unwrap();
    assert_eq!(user.name, OWNER_NAME);
    assert_eq!(user.code, "");
    assert_eq!(user.verified, false);
    assert!(user.verification_date.is_none());

    db::add_login_code_to_user(&dbh, OWNER_EMAIL, "qqrq", "new code")
        .await
        .unwrap();
    let user = db::get_user_by_uid(&dbh, 2).await.unwrap().unwrap();
    assert_eq!(user.name, OWNER_NAME);
    assert_eq!(user.code, "new code");
    assert_eq!(user.verified, false);
    assert!(user.verification_date.is_none());

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_update_user() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    let admin = get_user_by_uid(&dbh, 1).await.unwrap().unwrap();
    //println!("{admin:?}");
    assert_eq!(
        admin,
        User {
            id: admin.id.clone(),
            uid: 1,
            email: ADMIN_EMAIL.to_string(),
            password: String::from("should be hashed password"),
            name: ADMIN_NAME.to_string(),
            code: String::from("generated code"),
            process: String::from("register"),
            verified: false,
            registration_date: admin.registration_date,
            verification_date: None,
            code_generated_date: admin.code_generated_date,
            github: None,
            gitlab: None,
            linkedin: None,
            about: None
        }
    );

    db::update_user(
        &dbh,
        1,
        "New Name",
        "foogh",
        "foogl",
        "https://linkedin.com/",
        "about",
    )
    .await
    .unwrap();

    let admin = get_user_by_uid(&dbh, 1).await.unwrap().unwrap();
    assert_eq!(
        admin,
        User {
            id: admin.id.clone(),
            uid: 1,
            email: ADMIN_EMAIL.to_string(),
            password: String::from("should be hashed password"),
            name: "New Name".to_string(),
            code: String::from("generated code"),
            process: String::from("register"),
            verified: false,
            registration_date: admin.registration_date,
            verification_date: None,
            code_generated_date: admin.code_generated_date,
            github: Some(String::from("foogh")),
            gitlab: Some(String::from("foogl")),
            linkedin: Some(String::from("https://linkedin.com/")),
            about: Some(String::from("about")),
        }
    );

    db::save_password(&dbh, 1, "new password").await.unwrap();
    let admin = get_user_by_uid(&dbh, 1).await.unwrap().unwrap();
    assert_eq!(
        admin,
        User {
            id: admin.id.clone(),
            uid: 1,
            email: ADMIN_EMAIL.to_string(),
            password: String::from("new password"),
            name: "New Name".to_string(),
            code: String::from("generated code"),
            process: String::from("register"),
            verified: false,
            registration_date: admin.registration_date,
            verification_date: None,
            code_generated_date: admin.code_generated_date,
            github: Some(String::from("foogh")),
            gitlab: Some(String::from("foogl")),
            linkedin: Some(String::from("https://linkedin.com/")),
            about: Some(String::from("about")),
        }
    );

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_update_group() {
    let (dbh, db_name) = setup().await;
    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_groups_helper(&dbh).await;

    let group = db::get_group_by_gid(&dbh, 1).await.unwrap().unwrap();
    //println!("{group:?}");
    assert_eq!(
        group,
        Group {
            id: group.id.clone(),
            gid: 1,
            name: String::from("Rust Maven"),
            location: String::new(),
            description: String::new(),
            owner: 2,
            creation_date: group.creation_date,
        }
    );

    db::update_group(&dbh, 1, "New Name", "New Location", "New Description")
        .await
        .unwrap();

    let group = db::get_group_by_gid(&dbh, 1).await.unwrap().unwrap();
    //println!("{group:?}");
    assert_eq!(
        group,
        Group {
            id: group.id.clone(),
            gid: 1,
            name: String::from("New Name"),
            location: String::from("New Location"),
            description: String::from("New Description"),
            owner: 2,
            creation_date: group.creation_date,
        }
    );

    teardown(dbh, db_name).await;
}

#[async_test]
async fn test_db_update_event() {
    let (dbh, db_name) = setup().await;

    add_admin_helper(&dbh).await;
    add_owner_helper(&dbh).await;
    add_user_helper(&dbh).await;
    add_groups_helper(&dbh).await;
    add_events_helper(&dbh).await;

    let mut event = db::get_event_by_eid(&dbh, 1).await.unwrap().unwrap();
    //println!("{event:?}");
    assert_eq!(
        event,
        Event {
            id: event.id.clone(),
            eid: 1,
            title: String::from("First Conference"),
            description: String::new(),
            date: event.date,
            location: String::new(),
            group_id: 1,
            status: EventStatus::Published,
        }
    );

    event.title = String::from("New Title");
    event.description = String::from("New Description");
    event.location = String::from("New Location");

    db::update_event(&dbh, &event).await.unwrap();

    let updated_event = db::get_event_by_eid(&dbh, 1).await.unwrap().unwrap();
    //println!("{event:?}");
    assert_eq!(
        updated_event,
        Event {
            id: event.id.clone(),
            eid: 1,
            title: String::from("New Title"),
            description: String::from("New Description"),
            date: event.date,
            location: String::from("New Location"),
            group_id: 1,
            status: EventStatus::Published,
        }
    );

    teardown(dbh, db_name).await;
}
