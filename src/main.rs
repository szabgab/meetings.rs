#![allow(clippy::std_instead_of_core)]

#[macro_use]
extern crate rocket;

use std::env;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use rocket::form::Form;
use rocket::fs::NamedFile;
use rocket::http::CookieJar;
use rocket::serde::uuid::Uuid;
use rocket::{fairing::AdHoc, State};
use rocket_dyn_templates::{context, Template};

use markdown::message;

use serde::{Deserialize, Serialize};

use pbkdf2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Pbkdf2,
};

use meetings::{
    add_group, add_user, db, get_events_by_group_id, get_events_from_database, get_group_by_gid,
    get_groups_from_database, get_user_by_email, load_event, load_group, sendgrid, verify_code,
    EmailAddress, Group, User,
};

use surrealdb::engine::remote::ws::Client;
use surrealdb::Surreal;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Visitor {
    logged_in: bool,
    admin: bool,
    user: Option<User>,
}

impl Visitor {
    async fn new(
        cookies: &CookieJar<'_>,
        db: &State<Surreal<Client>>,
        myconfig: &State<MyConfig>,
    ) -> Self {
        let mut me = Self {
            logged_in: false,
            admin: false,
            user: None,
        };

        if let Some(cookie_user) = get_logged_in(cookies) {
            me.logged_in = true;
            if let Ok(user) = get_user_by_email(db, &cookie_user.email).await {
                me.user = user;
                //rocket::info!("email: {}", user.email);
                if myconfig.admins.contains(&cookie_user.email.clone()) {
                    me.admin = true;
                }
            }
        }

        me
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct MyConfig {
    base_url: String,

    #[serde(default = "get_empty_string")]
    sendgrid_api_key: String,

    admins: Vec<String>,
}

fn get_empty_string() -> String {
    String::new()
}

#[derive(Deserialize, Serialize, Debug)]
struct PublicConfig {
    google_analytics: String,
}

#[derive(FromForm)]
struct GroupForm<'r> {
    name: &'r str,
    location: &'r str,
    description: &'r str,
}

#[derive(FromForm)]
struct RegistrationForm<'r> {
    name: &'r str,
    email: &'r str,
    password: &'r str,
}

#[derive(FromForm)]
struct LoginForm<'r> {
    email: &'r str,
    password: &'r str,
}

#[derive(Serialize, Deserialize, Debug)]
struct CookieUser {
    email: String,
}

fn get_public_config() -> PublicConfig {
    let filename = "config.yaml";
    let raw_string = read_to_string(filename).unwrap();
    let data: PublicConfig = serde_yaml::from_str(&raw_string).expect("YAML parsing error");
    data
}

fn markdown2html(text: &str) -> Result<String, message::Message> {
    markdown::to_html_with_options(
        text,
        &markdown::Options {
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                ..markdown::CompileOptions::default()
            },
            ..markdown::Options::gfm()
        },
    )
}

fn get_logged_in(cookies: &CookieJar<'_>) -> Option<CookieUser> {
    if let Some(cookie) = cookies.get_private("meet-os") {
        let email = cookie.value();
        rocket::info!("cookie value received from user: {email}");
        return Some(CookieUser {
            email: email.to_owned(),
        });
    }
    None
}

#[get("/js/<file..>")]
async fn js_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/js/").join(file))
        .await
        .ok()
}

#[get("/")]
async fn index(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    rocket::info!("home");
    let config = get_public_config();
    let visitor = Visitor::new(cookies, db, myconfig).await;

    let events = match get_events_from_database(db).await {
        Ok(val) => val,
        Err(err) => {
            rocket::error!("Error: {err}");
            return Template::render(
                "message",
                context! {title: "Internal error", message: "Internal error", config, visitor},
            );
        }
    };

    let groups = match get_groups_from_database(db).await {
        Ok(val) => val,
        Err(err) => {
            rocket::error!("Error: {err}");
            return Template::render(
                "message",
                context! {title: "Internal error", message: "Internal error", config, visitor},
            );
        }
    };

    Template::render(
        "index",
        context! {
            title: "Meet-OS",
            events,
            groups,
            config,
            visitor,
        },
    )
}

#[get("/about")]
async fn about(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    Template::render(
        "about",
        context! {
            title: "About Meet-OS",
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[get("/privacy")]
async fn privacy(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    Template::render(
        "privacy",
        context! {
            title: "Privacy Policy",
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[get("/soc")]
async fn soc(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    Template::render(
        "soc",
        context! {
            title: "Standard of Conduct",
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[get("/login")]
async fn login_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    Template::render(
        "login",
        context! {
            title: "Login",
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[post("/login", data = "<input>")]
async fn login_post(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
    input: Form<LoginForm<'_>>,
) -> Template {
    rocket::info!("rocket login: {:?}", input.email);

    let config = get_public_config();
    let mut visitor = Visitor::new(cookies, db, myconfig).await;

    let email = input.email.to_lowercase().trim().to_owned();
    if !validator::validate_email(&email) {
        return Template::render(
            "message",
            context! {title: "Invalid email address", message: format!("Invalid email address <b>{}</b>. Please try again", input.email), config, visitor},
        );
    }

    let user = match get_user_by_email(db, &email).await {
        Ok(user) => user,
        Err(err) => {
            rocket::error!("Error: {err}");
            return Template::render(
                "message",
                context! {title: "No such user", message: format!("No user with address <b>{}</b>. Please try again", input.email), config, visitor},
            );
        }
    };

    let Some(user) = user else {
        return Template::render(
            "message",
            context! {title: "No such user", message: format!("No user with address <b>{}</b>. Please try again", input.email), config,visitor},
        );
    };

    rocket::info!("email: {}", user.email);

    let password = input.password.trim().as_bytes();

    let parsed_hash = match PasswordHash::new(&user.password) {
        Ok(val) => val,
        Err(err) => {
            rocket::error!("Error: {err}");
            return Template::render(
                "message",
                context! {title: "Internal error", message: "Internal error", config, visitor},
            );
        }
    };

    if Pbkdf2.verify_password(password, &parsed_hash).is_ok() {
        cookies.add_private(("meet-os", user.email)); // TODO this should be the user ID, right?

        // It seems despite calling add_private, the cookies will still return the old value so
        // for now we manually set the logged_in field
        visitor.logged_in = true;
        Template::render(
            "message",
            context! {title: "Welcome back", message: r#"Welcome back. <a href="/profile">profile</a>"#, config, visitor},
        )
    } else {
        Template::render(
            "message",
            context! {title: "Invalid password", message: "Invalid password", config, visitor},
        )
    }
}

#[get("/logout")]
async fn logout_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    // TODO shall we check if the cookie was even there?
    cookies.remove_private("meet-os");
    let visitor = Visitor::new(cookies, db, myconfig).await;
    Template::render(
        "message",
        context! {title: "Logged out", message: "We have logged you out from the system", config: get_public_config(), visitor},
    )
}

// #[post("/reset-password", data = "<input>")]
// async fn reset_password_post(
//     cookies: &CookieJar<'_>,
//     db: &State<Surreal<Db>>,
//     input: Form<LoginForm<'_>>,
//     myconfig: &State<MyConfig>,
// ) -> Template {
//     rocket::info!("rocket login: {:?}", input.email);

//     let email = input.email.to_lowercase().trim().to_owned();
//     if !validator::validate_email(&email) {
//         return Template::render(
//             "message",
//             context! {title: "Invalid email address", message: format!("Invalid email address <b>{}</b>. Please try again", input.email), config: get_public_config(), visitor},
//         );
//     }

//     let user: User = match get_user_by_email(db, &email).await {
//         Ok(user) => match user {
//             Some(user) => user,
//             None => {
//                 return Template::render(
//                     "message",
//                     context! {title: "No such user", message: format!("No user with address <b>{}</b>. Please try again", input.email), config: get_public_config(),visitor},
//                 )
//             }
//         },
//         Err(err) => {
//             rocket::error!("Error: {err}");
//             return Template::render(
//                 "message",
//                 context! {title: "No such user", message: format!("No user with address <b>{}</b>. Please try again", input.email), config: get_public_config(), visitor},
//             );
//         }
//     };

//     let process = "login";
//     let code = Uuid::new_v4();

//     match add_login_code_to_user(db, &email, process, code.to_string().as_str()).await {
//         Ok(_result) => (),
//         Err(err) => {
//             rocket::info!("Error while trying to add user {err}");
//             return Template::render(
//                 "message",
//                 context! {title: "Internal error", message: "Oups", config: get_public_config(), visitor,},
//             );
//         }
//     };

//     let base_url = &myconfig.base_url;

//     let subject = "Verify your Meet-OS login!";
//     let text = format!(
//         r#"Hi,
//     Someone used your email to try to login the Meet-OS web site.
//     If it was you, please <a href="{base_url}/verify/{process}/{code}">click on this link</a> to finish the login process.
//     <p>
//     <p>
//     If it was not you, we would like to apolozie. You don't need to do anything..
//     ";
//     "#
//     );

//     // TODO: read from some config file
//     let from = EmailAddress {
//         name: String::from("Meet OS"),
//         email: String::from("gabor@szabgab.com"),
//     };
//     let to_address = &EmailAddress {
//         name: user.name.clone(),
//         email: input.email.to_owned(),
//     };

//     if let Ok(email_file) = env::var("EMAIL_FILE") {
//         rocket::info!("email_file: {email_file}");
//         let mut file = File::create(email_file).unwrap();
//         writeln!(&mut file, "{}", &text).unwrap();
//     } else {
//         sendgrid(&myconfig.sendgrid_api_key, &from, to_address, subject, &text).await;
//     }

//     Template::render(
//         "message",
//         context! {title: "We sent you an email", message: format!("We sent you an email to <b>{}</b> Please click on the link to finish the login process.", to_address.email), config: get_public_config(), visitor},
//     )
// }

#[get("/register")]
async fn register_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    Template::render(
        "register",
        context! {
            title: "Register",
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[post("/register", data = "<input>")]
async fn register_post(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
    input: Form<RegistrationForm<'_>>,
) -> Template {
    rocket::info!("rocket input: {:?} {:?}", input.email, input.name);

    let config = get_public_config();
    let visitor = Visitor::new(cookies, db, myconfig).await;

    // email: lowerase, remove spaces from sides
    // validate format @
    let email = input.email.to_lowercase().trim().to_owned();
    if !validator::validate_email(&email) {
        return Template::render(
            "message",
            context! {title: "Invalid email address", message: format!("Invalid email address <b>{}</b> Please try again", input.email), config, visitor},
        );
    }

    let password = input.password.trim().as_bytes();
    let pw_min_length = 6;
    if password.len() < pw_min_length {
        return Template::render(
            "message",
            context! {title: "Invalid password", message: format!("The password must be at least {pw_min_length} characters long."), config, visitor},
        );
    }
    let process = "register";
    let code = Uuid::new_v4();
    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = match Pbkdf2.hash_password(password, &salt) {
        Ok(val) => val.to_string(),
        Err(err) => {
            rocket::error!("Error: {err}");
            return Template::render(
                "message",
                context! {title: "Invalid password", message: format!("The password must be at least {pw_min_length} characters long."), config, visitor},
            );
        }
    };

    let user = User {
        name: input.name.to_owned(),
        email,
        password: hashed_password,
        process: process.to_owned(),
        code: format!("{code}"),
        date: "date".to_owned(), // TODO get current timestamp
        verified: false,
    };
    match add_user(db, &user).await {
        Ok(result) => result,
        Err(err) => {
            rocket::info!("Error while trying to add user {err}");
            // TODO special reporting when the email is already in the system
            return Template::render(
                "message",
                context! {title: "Registration failed", message: format!("Could not register <b>{}</b>.", user.email), config, visitor},
            );
        }
    };

    let base_url = &myconfig.base_url;
    let subject = "Verify your Meet-OS registration!";
    let text = format!(
        r#"Hi,
    Someone used your email to register on the Meet-OS web site.
    If it was you, please <a href="{base_url}/verify/{process}/{code}">click on this link</a> to verify your email address.
    <p>
    <p>
    If it was not you, we would like to apolozie. You don't need to do anything. We'll discard your registration if it is not validated.
    ";
    "#
    );

    // TODO: read from some config file
    let from = EmailAddress {
        name: String::from("Meet OS"),
        email: String::from("gabor@szabgab.com"),
    };
    let to_address = &EmailAddress {
        name: input.name.to_owned(),
        email: input.email.to_owned(),
    };

    if let Ok(email_file) = env::var("EMAIL_FILE") {
        rocket::info!("email_file: {email_file}");
        let mut file = File::create(email_file).unwrap();
        writeln!(&mut file, "{}", &text).unwrap();
    } else {
        // TODO display some error if the sendgrid key is empty
        // TODO display some error if the email sending failed
        sendgrid(
            &myconfig.sendgrid_api_key,
            &from,
            to_address,
            subject,
            &text,
        )
        .await;
    }

    Template::render(
        "message",
        context! {title: "We sent you an email", message: format!("We sent you an email to <b>{}</b> Please check your inbox and verify your email address.", to_address.email), config, visitor},
    )

    // Template::render(
    //     "register",
    //     context! {title: "Register", config: get_public_config()},
    // )
}

// TODO limit the possible values for the process to register and login
#[get("/verify/<process>/<code>")]
async fn verify(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
    process: &str,
    code: &str,
) -> Template {
    rocket::info!("process: {process}, code: {code}");

    let config = get_public_config();
    let mut visitor = Visitor::new(cookies, db, myconfig).await;

    // TODO take the process into account at the verification
    if let Ok(Some(user)) = verify_code(db, process, code).await {
        rocket::info!("verified: {}", user.email);
        cookies.add_private(("meet-os", user.email.clone())); // TODO this should be the user ID, right?
        let (title, message) = match process {
            "register" => ("Thank you for registering", "Your email was verified."),
            "login" => ("Welcome back", "Welcome back"),
            _ => ("Oups", "Big opus and TODO"),
        };

        // take into account the newly set cookie value
        visitor.logged_in = true;
        return Template::render(
            "message",
            context! {title: title, message: message, config, visitor},
        );
    }
    Template::render(
        "message",
        context! {title: "Invalid code", message: format!("Invalid code <b>{code}</b>"), config, visitor},
    )
}

#[get("/profile")]
async fn show_profile(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    let config = get_public_config();
    let visitor = Visitor::new(cookies, db, myconfig).await;

    if let Some(cookie) = cookies.get_private("meet-os") {
        let email = cookie.value();
        rocket::info!("cookie value received from user: {email}");
        if let Ok(Some(user)) = get_user_by_email(db, email).await {
            rocket::info!("email: {}", user.email);
            return Template::render(
                "profile",
                context! {title: "Profile", user: user, config, visitor},
            );
        }
    }

    Template::render(
        "message",
        context! {title: "Not logged in", message: format!("It seems you are not logged in"), config, visitor},
    )
}

#[get("/event/<id>")]
async fn event_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
    id: usize,
) -> Template {
    let event = load_event(id);
    let group = load_group(event.group_id);

    let body = markdown2html(&event.body).unwrap();

    Template::render(
        "event",
        context! {
            title: &event.title,
            event: &event,
            body: body,
            group: group,
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[get("/group/<gid>")]
async fn group_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
    gid: usize,
) -> Template {
    rocket::info!("group_get: {gid}");
    let config = get_public_config();
    let visitor = Visitor::new(cookies, db, myconfig).await;

    let group = match get_group_by_gid(db, gid).await {
        Ok(group) => match group {
            Some(group) => group,
            None => {
                return Template::render(
                    "message",
                    context! {title: "No such group", message: "No such group", config, visitor},
                )
            } // TODO 404
        },
        Err(err) => {
            rocket::error!("Error: {err}");
            return Template::render(
                "message",
                context! {title: "Internal error", message: "Internal error", config, visitor},
            );
        }
    };

    let events = get_events_by_group_id(db, gid).await;

    let description = markdown2html(&group.description).unwrap();

    Template::render(
        "group",
        context! {
            title: &group.name,
            group: &group,
            description: description,
            events: events,
            config,
            visitor,
        },
    )
}

#[get("/groups")]
async fn groups_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    let config = get_public_config();
    let visitor = Visitor::new(cookies, db, myconfig).await;

    match get_groups_from_database(db).await {
        Ok(groups) => Template::render(
            "groups",
            context! {title: "Groups", groups: groups, config, visitor},
        ),
        Err(err) => {
            rocket::error!("Error {err}");
            Template::render(
                "message",
                context! {title: "Internal error", message: "Internal error", config, visitor},
            )
        }
    }

    // if let Ok(groups) = get_groups_from_database(db).await {
    //     return Template::render(
    //         "groups",
    //         context! {title: "Groups", groups: groups, config: get_public_config(), visitor},
    //     );
    // }
    // Template::render(
    //     "message",
    //     context! {title: "Internal error", message: "Internal error", config: get_public_config(), visitor},
    // )
}

#[get("/admin")]
async fn admin(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    Template::render(
        "admin",
        context! {
            title: "Admin",
            config: get_public_config(),
            visitor: Visitor::new(cookies, db, myconfig).await,
        },
    )
}

#[get("/create-group")]
async fn create_group_get(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
) -> Template {
    let config = get_public_config();
    let visitor = Visitor::new(cookies, db, myconfig).await;

    if !visitor.logged_in {
        return Template::render(
            "message",
            context! {title: "Not logged in", message: format!("It seems you are not logged in"), config, visitor},
        );
    };

    let user = visitor.user.clone().unwrap();

    rocket::info!("cookie value received from user: {}", user.email);
    if !visitor.admin {
        return Template::render(
            "message",
            context! {title: "Unauthorized", message: "Unauthorized", config, visitor},
        );
    };

    Template::render(
        "create_group",
        context! {title: "Create Group", user: user, config, visitor},
    )
}

#[post("/create-group", data = "<input>")]
async fn create_group_post(
    cookies: &CookieJar<'_>,
    db: &State<Surreal<Client>>,
    myconfig: &State<MyConfig>,
    input: Form<GroupForm<'_>>,
) -> Template {
    rocket::info!("create_group_post: {:?}", input.name);
    let config = get_public_config();

    let visitor = Visitor::new(cookies, db, myconfig).await;

    if !visitor.logged_in {
        return Template::render(
            "message",
            context! {title: "Not logged in", message: format!("It seems you are not logged in"), config, visitor},
        );
    };

    rocket::info!(
        "cookie value received from user: {}",
        visitor.user.clone().unwrap().email
    );

    if !visitor.admin {
        return Template::render(
            "message",
            context! {title: "Unauthorized", message: "Unauthorized", config, visitor},
        );
    }

    let gid = match get_groups_from_database(db).await {
        Ok(groups) => groups.len().saturating_add(1),
        Err(err) => {
            rocket::info!("Error while trying to add group {err}");
            1 // TODO return internal error message
        }
    };

    rocket::info!("group_id: {gid}");
    let group = Group {
        name: input.name.to_owned(),
        location: input.location.to_owned(),
        description: input.description.to_owned(),
        gid,
    };

    match add_group(db, &group).await {
        Ok(_result) => Template::render(
            "message",
            context! {title: "Group created", message: format!(r#"Group <b><a href="/group/{}/{}</a></b>created"#, gid, group.name), config, visitor},
        ),
        Err(err) => {
            rocket::info!("Error while trying to add group {err}");
            Template::render(
                "message",
                context! {title: "Failed", message: format!("Could not add <b>{}</b>.", group.name), config, visitor},
            )
        }
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount(
            "/",
            routes![
                about,
                admin,
                create_group_get,
                create_group_post,
                event_get,
                groups_get,
                group_get,
                index,
                js_files,
                logout_get,
                login_get,
                login_post,
                privacy,
                register_get,
                register_post,
                show_profile,
                soc,
                verify
            ],
        )
        .attach(Template::fairing())
        .attach(db::fairing())
        .attach(AdHoc::config::<MyConfig>())
}
