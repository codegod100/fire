#[macro_use]
extern crate rocket;
use rocket::http::{CookieJar, Status};
use rocket::response::{Flash, Redirect};

use rocket_dyn_templates::{context, tera::Tera, Template};

#[get("/")]
fn index(jar: &CookieJar<'_>) -> Result<Redirect, Template> {
    match jar.get_private("user_id") {
        Some(c) => Ok(Redirect::to(uri!(greeter))),
        None => Err(Template::render("login", context! {})),
    }
}

#[post("/")]
fn post_login(jar: &CookieJar<'_>) -> Redirect {
    jar.add_private(("user_id", "admin"));
    Redirect::to(uri!(greeter))
}

#[get("/greeter")]
fn greeter(jar: &CookieJar<'_>) -> Result<Template, Redirect> {
    match jar.get_private("user_id") {
        Some(c) => Ok(Template::render(
            "index",
            context! {
                name: c.value()
            },
        )),
        None => Err(Redirect::to(uri!(index))),
    }
}

#[post("/greeter")]
fn post_greeter(jar: &CookieJar<'_>) -> Redirect {
    jar.remove_private("user_id");
    Redirect::to(uri!(greeter))
}
#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, greeter, post_login, post_greeter])
        .attach(Template::fairing())
}
