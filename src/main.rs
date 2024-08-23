#[macro_use]
extern crate rocket;
use rocket::http::{CookieJar, Status};
use rocket::response::{Flash, Redirect};
use rocket_dyn_templates::{context, tera::Tera, Template};
// use serde::Serialize;

mod db;

#[get("/")]
fn index(jar: &CookieJar<'_>) -> Template {
    match jar.get_private("user_id") {
        Some(c) => Template::render(
            "index",
            context! {
                name: c.value(),

            },
        ),
        None => Template::render("login", context! {}),
    }
}

#[post("/")]
fn post_login(jar: &CookieJar<'_>) -> Redirect {
    jar.add_private(("user_id", "admin"));
    Redirect::to(uri!(index))
}

// #[get("/greeter")]
// fn greeter(jar: &CookieJar<'_>) -> Result<Template, Redirect> {
//     match jar.get_private("user_id") {
//         Some(c) => Ok(Template::render(
//             "index",
//             context! {
//                 name: c.value()
//             },
//         )),
//         None => Err(Redirect::to(uri!(index))),
//     }
// }

#[post("/logout")]
fn logout(jar: &CookieJar<'_>) -> Redirect {
    jar.remove_private("user_id");
    Redirect::to(uri!(index))
}
#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, post_login, logout])
        .attach(Template::fairing())
}
