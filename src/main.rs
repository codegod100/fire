#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{context, tera::Tera, Template};

#[get("/")]
fn index() -> &'static str {
    "hello worlds"
}

#[get("/greeter/<name>")]
fn greeter(name: &str) -> Template {
    Template::render(
        "index",
        context! {
            name: Some(name)
        },
    )
}
#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, greeter])
        .attach(Template::fairing())
}
