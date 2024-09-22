use crate::test_lib::{
    check_html, check_message, check_not_the_owner, check_unauthorized, check_unprocessable,
    params, setup_admin, setup_owner, setup_user, TestRunner, ADMIN_EMAIL, OWNER_EMAIL, USER_EMAIL,
    USER_NAME,
};
use rocket::http::{ContentType, Status};

// GET /create-group show form
// POST /create-group verify name, add group to database
// GET /groups  list all the groups from the database

// guest cannot access the /create-group pages
// regular user cannot access the /create-group pages
// only admin user can access the /create-group pages
// everyone can access the /groups page

#[test]
fn create_group_by_admin() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);

    // Access the Group creation page with authorized user
    let res = tr
        .client
        .get("/admin/create-group?uid=2")
        .private_cookie(("meet-os", ADMIN_EMAIL))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    check_html!(&html, "title", "Create Group");
    check_html!(&html, "h1", "Create Group");

    // Create a Group
    let res = tr
        .client
        .post("/admin/create-group")
        .header(ContentType::Form)
        .body(params!([
            ("name", "Rust Maven"),
            ("location", "Virtual"),
            (
                "description",
                "Text with [link](https://rust.code-maven.com/)",
            ),
            ("owner", "2"),
        ]))
        .private_cookie(("meet-os", ADMIN_EMAIL))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    // List the groups
    let res = tr.client.get("/groups").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    assert!(html.contains(r#"<li><a href="/group/1">Rust Maven</a></li>"#));
    check_html!(&html, "title", "Groups");
    check_html!(&html, "h1", "Groups");

    let res = tr
        .client
        .post("/admin/create-group")
        .header(ContentType::Form)
        .body(params!([
            ("name", "Python Maven"),
            ("location", "Other"),
            ("description", "Text with [link](https://code-maven.com/)"),
            ("owner", "2"),
        ]))
        .private_cookie(("meet-os", ADMIN_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);

    // List the groups
    let res = tr.client.get("/groups").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    assert!(html.contains(r#"<li><a href="/group/1">Rust Maven</a></li>"#));
    assert!(html.contains(r#"<li><a href="/group/2">Python Maven</a></li>"#));
    check_html!(&html, "title", "Groups");
    check_html!(&html, "h1", "Groups");
}

#[test]
fn create_group_unauthorized() {
    let tr = TestRunner::new();

    setup_user(&tr.client, &tr.email_folder);

    // Access the Group creation page with unauthorized user
    let res = tr
        .client
        .get("/admin/create-group?uid=1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    check_unauthorized!(res);

    // Create group should fail
    let res = tr
        .client
        .post("/admin/create-group")
        .body(params!([
            ("name", "Rust Maven"),
            ("location", "Virtual"),
            ("description", "nope"),
            ("owner", "1"),
        ]))
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    check_unauthorized!(res);
}

#[test]
fn create_group_guest() {
    let tr = TestRunner::new();

    let res = tr.client.get("/groups").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    assert!(!html.contains("/group/")); // No link to any group
    check_html!(&html, "title", "Groups");
    check_html!(&html, "h1", "Groups");

    // List the groups
    let res = tr.client.get("/groups").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    assert!(!html.contains("/group/1"));
    check_html!(&html, "title", "Groups");
    check_html!(&html, "h1", "Groups");
}

#[test]
fn get_join_group_not_existing_group_as_user() {
    let tr = TestRunner::new();

    setup_owner(&tr.client, &tr.email_folder);

    let res = tr
        .client
        .get("/join-group?gid=20")
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    check_message!(
        &html,
        "No such group",
        "There is not group with id <b>20</b>"
    );
}

#[test]
fn get_join_group_as_user() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);
    setup_user(&tr.client, &tr.email_folder);
    tr.create_group_helper("First Group", 2);
    tr.logout();

    // user joins group
    let res = tr
        .client
        .get("/join-group?gid=1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    check_message!(
        &html,
        "Membership",
        r#"User added to <a href="/group/1">group</a>"#
    );

    // check if user is listed on the group page
    let res = tr.client.get("/group/1").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    check_html!(&html, "title", "First Group");
    check_html!(&html, "h1", "First Group");
    assert!(html.contains(r#"<h2 class="title is-4">Members</h2>"#));
    let expected = format!(r#"<a href="/user/3">{USER_NAME}</a>"#);
    assert!(html.contains(&expected));

    // visit the group page as a member of the group
    let res = tr
        .client
        .get("/group/1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    //assert_eq!(html, "");
    check_html!(&html, "title", "First Group");
    check_html!(&html, "h1", "First Group");
    assert!(html.contains(r#"<h2 class="title is-4">Members</h2>"#));
    let expected = format!(r#"<a href="/user/3">{USER_NAME}</a>"#);
    assert!(html.contains(&expected));
    assert!(html.contains(r#"You are a member."#));
    assert!(html.contains(
        r#"<a href="/leave-group?gid=1"><button class="button is-link">leave group</button></a>"#
    ));

    // try to join the same group again - should fail
    let res = tr
        .client
        .get("/join-group?gid=1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    // assert_eq!(html, "");
    check_html!(&html, "title", "You are already a member of this group");
    check_html!(&html, "h1", "You are already a member of this group");
    assert!(html
        .contains(r#"You are already a member of the <a href="/group/1">First Group</a> group"#));

    // leave group
    let res = tr
        .client
        .get("/leave-group?gid=1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    check_message!(
        &html,
        "Membership",
        r#"User removed from <a href="/group/1">group</a>"#
    );

    // See that user is NOT listed on the group page any more
    let res = tr.client.get("/group/1").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    //assert_eq!(html, "");
    check_html!(&html, "title", "First Group");
    check_html!(&html, "h1", "First Group");
    assert!(html.contains(r#"<h2 class="title is-4">Members</h2>"#));
    assert!(!html.contains(USER_NAME));
    assert!(!html.contains("/user/3"));
}

#[test]
fn get_join_group_as_owner() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);
    tr.create_group_helper("First Group", 2);

    let res = tr
        .client
        .get("/join-group?gid=1")
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    check_message!(
        &html,
        "You are the owner of this group",
        r#"You cannot join a group you own."#
    );
}

#[test]
fn get_leave_not_existing_group() {
    let tr = TestRunner::new();

    setup_owner(&tr.client, &tr.email_folder);

    let res = tr
        .client
        .get("/leave-group?gid=20")
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    check_message!(
        &html,
        "No such group",
        "The group ID <b>20</b> does not exist."
    );
}

#[test]
fn get_leave_group_as_owner() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);
    tr.create_group_helper("First Group", 2);

    let res = tr
        .client
        .get("/leave-group?gid=1")
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    //assert_eq!(html, "");
    check_html!(&html, "title", "You are the owner of this group");
    check_html!(&html, "h1", "You are the owner of this group");
    assert!(html.contains(r#"You cannot leave a group you own."#));
}

#[test]
fn get_leave_group_user_does_not_belong_to() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);
    setup_user(&tr.client, &tr.email_folder);
    tr.create_group_helper("First Group", 2);

    let res = tr
        .client
        .get("/leave-group?gid=1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    //assert_eq!(html, "");
    check_html!(&html, "title", "You are not a member of this group");
    check_html!(&html, "h1", "You are not a member of this group");
    assert!(html.contains(r#"You cannot leave a group where you are not a member."#));
}

#[test]
fn get_edit_group_user_no_such_group() {
    let tr = TestRunner::new();

    setup_owner(&tr.client, &tr.email_folder);

    let res = tr
        .client
        .get("/edit-group?gid=1")
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    //assert_eq!(html, "");
    check_html!(&html, "title", "No such group");
    check_html!(&html, "h1", "No such group");
    assert!(html.contains("Group <b>1</b> does not exist"));
}

#[test]
fn get_edit_group_user_is_not_the_owner() {
    let tr = TestRunner::new();

    tr.setup_for_groups();

    let res = tr
        .client
        .get("/edit-group?gid=1")
        .private_cookie(("meet-os", USER_EMAIL))
        .dispatch();
    check_not_the_owner!(res);
}

#[test]
fn get_edit_group_by_owner() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);
    tr.create_group_helper("First Group", 2);

    let res = tr
        .client
        .get("/edit-group?gid=1")
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    //assert_eq!(html, "");
    check_html!(&html, "title", "Edit Group");
    check_html!(&html, "h1", "Edit Group");
    assert!(html.contains(r#"<form method="POST" action="/edit-group">"#));
    assert!(html.contains(r#"<input type="hidden" name="gid" value="1">"#));
    assert!(html.contains(r#"Name: <input name="name" id="name" type="text" value="First Group">"#));
    assert!(
        html.contains(r#"Location: <input name="location" id="location" type="text" value="">"#)
    );
    assert!(html.contains(r#"Description (<a href="/markdown">Markdown</a>): <textarea name="description" id="description"></textarea>"#));
    assert!(html.contains(r#"<input type="submit" value="Save">"#));
}

#[test]
fn get_group_that_does_not_exist() {
    let tr = TestRunner::new();

    let res = tr.client.get("/group/42").dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    //assert_eq!(html, "");
    check_html!(&html, "title", "No such group");
    check_html!(&html, "h1", "No such group");
    assert!(html.contains("The group <b>42</b> does not exist."));
}

#[test]
fn post_edit_group_user_missing_gid() {
    let tr = TestRunner::new();

    setup_owner(&tr.client, &tr.email_folder);

    let res = tr
        .client
        .post("/edit-group")
        .header(ContentType::Form)
        .private_cookie(("meet-os", OWNER_EMAIL))
        .dispatch();
    check_unprocessable!(res);
}

#[test]
fn post_edit_group_user_no_such_group() {
    let tr = TestRunner::new();

    setup_owner(&tr.client, &tr.email_folder);

    let res = tr
        .client
        .post("/edit-group")
        .header(ContentType::Form)
        .private_cookie(("meet-os", OWNER_EMAIL))
        .body(params!([
            ("gid", "1"),
            ("name", "Update"),
            ("location", "Virtual"),
            ("description", "",),
        ]))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    //assert_eq!(html, "");
    check_html!(&html, "title", "No such group");
    check_html!(&html, "h1", "No such group");
    assert!(html.contains("Group <b>1</b> does not exist"));
}

#[test]
fn post_edit_group_by_user_not_owner() {
    let tr = TestRunner::new();

    tr.setup_for_groups();

    let res = tr
        .client
        .post("/edit-group")
        .header(ContentType::Form)
        .private_cookie(("meet-os", USER_EMAIL))
        .body(params!([
            ("gid", "1"),
            ("name", "Updated name"),
            ("location", "Local"),
            ("description", "Some group"),
        ]))
        .dispatch();

    check_not_the_owner!(res);
}

#[test]
fn post_edit_group_user_not_owner() {
    let tr = TestRunner::new();

    tr.setup_for_groups();

    let res = tr
        .client
        .post("/edit-group")
        .header(ContentType::Form)
        .private_cookie(("meet-os", USER_EMAIL))
        .body(params!([
            ("gid", "1"),
            ("name", "Updated name"),
            ("location", "Local"),
            ("description", "Some group"),
        ]))
        .dispatch();
    check_not_the_owner!(res);
}

#[test]
fn post_edit_group_owner() {
    let tr = TestRunner::new();

    setup_admin(&tr.client, &tr.email_folder);
    setup_owner(&tr.client, &tr.email_folder);
    //setup_foo1(&tr.client, &tr.email_folder);
    tr.create_group_helper("First Group", 2);
    tr.logout();

    let res = tr
        .client
        .post("/edit-group")
        .header(ContentType::Form)
        .private_cookie(("meet-os", OWNER_EMAIL))
        .body(params!([
            ("gid", "1"),
            ("name", "Updated name"),
            ("location", "Local"),
            ("description", "Some group"),
        ]))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    check_message!(
        &html,
        "Group updated",
        r#"Check out the <a href="/group/1">group</a>"#
    );

    // check if the group was updated
    let res = tr.client.get("/group/1").dispatch();
    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();
    check_html!(&html, "title", "Updated name");
    check_html!(&html, "h1", "Updated name");
    assert!(html.contains(r#"<h2 class="title is-4">Members</h2>"#));
    assert!(html.contains(r#"No members in this group."#));
    assert!(html.contains(r#"<p>Some group</p>"#));
    assert!(html.contains(r#"<b>Location</b>: Local"#));
}
