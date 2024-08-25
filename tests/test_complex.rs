use utilities::{register_user_helper, run_external};

#[test]
fn test_complex() {
    run_external(|port, email_folder| {
        let client = reqwest::blocking::Client::new();
        let url = format!("http://localhost:{port}/");

        // main page
        let res = client.get(format!("{url}")).send().unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(!html.contains("<h2>Events</h2>"));
        assert!(!html.contains("<h2>Groups</h2>"));

        let admin_name = "Admin";
        let admin_email = "admin@meet-os.com";
        let admin_password = "123456";
        let admin_cookie_str = register_user_helper(
            &client,
            &url,
            admin_name,
            admin_email,
            admin_password,
            &email_folder,
        );

        let owner_name = "Owner";
        let owner_email = "owner@meet-os.com";
        let owner_password = "123456";
        let owner_cookie_str = register_user_helper(
            &client,
            &url,
            owner_name,
            &owner_email,
            &owner_password,
            &email_folder,
        );

        // profile is not listing the any groups
        let res = client
            .get(format!("{url}/profile"))
            .header("Cookie", format!("meet-os={owner_cookie_str}"))
            .send()
            .unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(!html.contains("Owned Groups"));
        assert!(!html.contains("Group Membership"));

        let group_name = "Rust Maven";
        let res = client
            .post(format!("{url}/admin/create-group"))
            .form(&[
                ("name", group_name),
                ("location", "Virtual"),
                ("description", ""),
                ("owner", "2"),
            ])
            .header("Cookie", format!("meet-os={admin_cookie_str}"))
            .send()
            .unwrap();
        assert_eq!(res.status(), 200);

        // The profile now lists the group for the owner
        let res = client
            .get(format!("{url}/profile"))
            .header("Cookie", format!("meet-os={owner_cookie_str}"))
            .send()
            .unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(html.contains("Owned Groups"));
        assert!(!html.contains("Group Membership"));
        assert!(html.contains(r#"<a href="/group/1">Rust Maven</a>"#));
        //assert_eq!(html, "");

        // TODO Add events
        let event_title = "The first meeting";
        let res = client
            .post(format!("{url}/edit-event"))
            .form(&[
                ("gid", "1"),
                ("offset", "-180"),
                ("title", event_title),
                ("location", "Virtual"),
                ("description", ""),
                ("date", "2030-01-01 10:10"),
            ])
            .header("Cookie", format!("meet-os={owner_cookie_str}"))
            .send()
            .unwrap();
        assert_eq!(res.status(), 200);

        // main page
        let res = client.get(format!("{url}")).send().unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(html.contains("<h2>Events</h2>"));
        assert!(html.contains("<h2>Groups</h2>"));
        assert!(html.contains(format!(r#"<a href="/group/1">{group_name}</a>"#).as_str()));
        assert!(html.contains(format!(r#"<a href="/event/1">{event_title}</a>"#).as_str()));

        // groups page
        let res = client.get(format!("{url}/groups")).send().unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(html.contains(format!(r#"<a href="/group/1">{group_name}</a>"#).as_str()));

        // events page
        let res = client.get(format!("{url}/events")).send().unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(html.contains(format!(r#"<a href="/event/1">{event_title}</a>"#).as_str()));

        // TODO check event pages
        let res = client.get(format!("{url}/event/1")).send().unwrap();
        assert_eq!(res.status(), 200);
        let html = res.text().unwrap();
        assert!(html.contains(format!(r#"<title>{event_title}</title>"#).as_str()));
        assert!(html.contains(format!(r#"<p class="title">{event_title}</p>"#).as_str()));
        assert!(
            html.contains(format!(r#"Organized by <a href="/group/1">{group_name}</a>."#).as_str())
        );

        // TODO change event
        // TODO check event pages
    });
}
