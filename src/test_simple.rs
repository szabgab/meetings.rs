use rocket::http::{ContentType, Status};
use rocket::local::blocking::Client;
use utilities::{check_guest_menu, check_html, check_user_menu, read_code_from_email};

#[test]
fn test_simple() {
    run_inprocess(|email_folder, client| {
        let email = "foo@meet-os.com";

        // main page
        let res = client.get("/").dispatch();
        assert_eq!(res.status(), Status::Ok);
        assert_eq!(
            res.headers().get_one("Content-Type").unwrap(),
            "text/html; charset=utf-8"
        );

        let html = res.into_string().unwrap();
        check_html(&html, "title", "Meet-OS");
        check_html(&html, "h1", "Welcome to the Meet-OS meeting server");
        assert!(!html.contains("<h2>Events</h2>"));
        assert!(!html.contains("<h2>Groups</h2>"));
        check_guest_menu(&html);

        // register user
        let res = client
            .post("/register")
            .header(ContentType::Form)
            .body("name=Foo Bar&email=foo@meet-os.com&password=123456")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        assert!(res.headers().get_one("set-cookie").is_none());

        let html = res.into_string().unwrap();
        check_html(&html, "title", "We sent you an email");
        assert!(html.contains("We sent you an email to <b>foo@meet-os.com</b> Please check your inbox and verify your email address."));
        check_guest_menu(&html);

        // validate the email
        let (uid, code) = read_code_from_email(&email_folder, "0.txt");
        let res = client.get(format!("/verify-email/{uid}/{code}")).dispatch();
        assert_eq!(res.status(), Status::Ok);

        let html = res.into_string().unwrap();
        check_html(&html, "title", "Thank you for registering");
        assert!(html.contains("Your email was verified."));
        check_user_menu(&html);

        // Access the profile with the cookie
        check_profile_page_in_process(&client, email, "Foo Bar");
        //check_profile_page_in_process(&client, "foo@meet-os.com", "");

        // register with same email should fail
        let res = client
            .post("/register")
            .header(ContentType::Form)
            .body("name=Foo Bar&email=foo@meet-os.com&password=123456")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Registration failed");
        assert!(html.contains("Could not register <b>foo@meet-os.com</b>"));

        // register with invalid email address
        let res = client
            .post("/register")
            .header(ContentType::Form)
            .body("name=Foo Bar&email=meet-os.com&password=123456")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Invalid email address");
        assert!(html.contains("Invalid email address <b>meet-os.com</b> Please try again"));

        // register name too long
        let res = client
            .post("/register")
            .header(ContentType::Form)
            .body("name=QWERTYUIOPASDFGHJKLZXCVBNM QWERTYUIOPASDFGHJKLZXCVBNM&email=long@meet-os.com&password=123456")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Name is too long");
        assert!(html.contains(
            "Name is too long. Max 50 while the current name is 53 long. Please try again."
        ));

        // edit profile page invalid github account
        let res = client
            .post("/edit-profile")
            .private_cookie(("meet-os", email))
            .header(ContentType::Form)
            .body("name=XX&github=szabgab*&gitlab=&linkedin&about=")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Invalid GitHub username");
        assert!(html.contains(r#"The GitHub username `szabgab*` is not valid."#));

        // edit profile page invalid gitlab account
        let res = client
            .post("/edit-profile")
            .private_cookie(("meet-os", email))
            .header(ContentType::Form)
            .body("name=XX&github=&gitlab=foo*bar&linkedin=&about=")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Invalid GitLab username");
        assert!(html.contains(r#"The GitLab username `foo*bar` is not valid."#));

        let res = client
            .post("/edit-profile")
            .private_cookie(("meet-os", email))
            .header(ContentType::Form)
            .body("name=XX&github=&gitlab=&linkedin=szabgab&about=")
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Invalid LinkedIn profile link");
        assert!(html.contains(r#"The LinkedIn profile link `szabgab` is not valid."#));

        // TODO test the validation of the other fields as well!

        //assert_eq!(html, "");

        // edit profile page
        // verify that if we submit html tags to the about field, those are properly escaped in the result
        let res = client
            .post("/edit-profile")
            .private_cookie(("meet-os", email))
            .header(ContentType::Form)
            .body("name=Luis XI&github=szabgab&gitlab=szabgab&linkedin=https://www.linkedin.com/in/szabgab/&about=* text\n* more\n* [link](https://meet-os.com/)\n* <b>bold</b>\n* <a href=\"https://meet-os.com/\">bad link</a>\n")
            .dispatch();

        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Profile updated");
        assert!(html.contains(r#"Check out the <a href="/profile">profile</a> and how others see it <a href="/user/1">Luis XI</a>"#));

        // Check updated profile
        let res = client
            .get("/profile")
            .private_cookie(("meet-os", email))
            .dispatch();

        assert_eq!(res.status(), Status::Ok);
        let html = res.into_string().unwrap();
        check_html(&html, "title", "Profile");
        assert!(html.contains(r#"<div><a href="https://github.com/szabgab">GitHub</a></div>"#));
        assert!(html.contains(r#"<div><a href="https://gitlab.com/szabgab">GitLab</a></div>"#));

        // TODO: do we need to escape the characters when we submit them in the test or is this really what should be expected?
        assert!(html.contains(r#"<div><a href="https:&#x2F;&#x2F;www.linkedin.com&#x2F;in&#x2F;szabgab&#x2F;">LinkedIn</a></div>"#));
        //assert!(html.contains(r#"<div><a href="https:://www.linkedin.com/in/szabgab/">LinkedIn</a></div>"#));

        eprintln!("{html}");
        //assert_eq!(html, "");
        assert!(html.contains(
            r#"<div><ul>
<li>text</li>
<li>more</li>
<li><a href="https://meet-os.com/">link</a></li>
<li>&lt;b&gt;bold&lt;/b&gt;</li>
<li>&lt;a href=&quot;https://meet-os.com/&quot;&gt;bad link&lt;/a&gt;</li>
</ul>
</div>"#
        ));

        // TODO resend code?
        // TODO reset password?
    });
}

pub fn run_inprocess(func: fn(std::path::PathBuf, Client)) {
    use rocket::config::Config;

    let tmp_dir = tempfile::tempdir().unwrap();
    println!("tmp_dir: {:?}", tmp_dir);
    let email_folder = tmp_dir.path().join("emails");
    let db_name = format!("test-name-{}", rand::random::<f64>());
    let db_namespace = format!("test-namespace-{}", rand::random::<f64>());

    let provider = Config::figment()
        .merge(("database_namespace", &db_namespace))
        .merge(("database_name", &db_name))
        .merge(("email", "Folder"))
        .merge(("email_folder", email_folder.to_str().unwrap()));

    let app = super::rocket().configure(provider);
    let client = Client::tracked(app).unwrap();

    func(email_folder, client);
}

pub fn check_profile_page_in_process(client: &Client, email: &str, h1: &str) {
    let res = client
        .get("/profile")
        .private_cookie(("meet-os", email.to_owned()))
        .dispatch();

    assert_eq!(res.status(), Status::Ok);
    let html = res.into_string().unwrap();

    if h1.is_empty() {
        check_html(&html, "title", "Not logged in");
        assert!(html.contains("It seems you are not logged in"));
    } else {
        check_html(&html, "title", "Profile");
        check_html(&html, "h1", h1);
    }
}
