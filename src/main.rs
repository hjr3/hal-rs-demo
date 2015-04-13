#![feature(phase)]

extern crate nickel;
extern crate hal;
extern crate rustc_serialize as serialize;
extern crate postgres;
extern crate time;
#[macro_use] extern crate nickel_macros;

use nickel::{Nickel, Request, Response, HttpRouter, Continue, Halt, Action};
use nickel::{NickelError, mimes};
use hal::{Link, Resource, ToHal};
use serialize::json::{ToJson, Json};
use postgres::{Connection, SslMode, Row, Rows, Statement, ToSql, Column};
use postgres::types::{Type};
use std::{os, env};
use std::str::FromStr;
use nickel::status::StatusCode::{NotFound, BadRequest};

struct Order {
    order_id: i32,
    total: f64,
    currency: String,
    status: String
}

impl ToHal for Order {
    fn to_hal(self) -> Resource {
        Resource::with_self(format!("https://www.example.com/orders/{}", self.order_id).as_slice())
            .add_state("total", self.total)
            .add_state("currency", self.currency)
            .add_state("status", self.status)
    }
}

fn get_option(key: &str, default: &str) -> String {
    match env::var(key) {
        Ok(val) => val.parse().unwrap(),
        Err(..) => default.to_string()
    }
}

fn connect() -> Connection {
    let host = get_option("DBHOST", "localhost");
    let port = get_option("DBPORT", "15432");
    let user = get_option("DBUSER", "myapp");
    let password = get_option("DBPASS", "dbpass");
    let dbname = get_option("DBNAME", "myapp");

    let dsn = format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, dbname);
    Connection::connect(dsn.as_slice(), &SslMode::None).unwrap()
}

// error handling is not working in nickel at the moment
//
//fn not_found_handler(message: String, response: &mut Response) {
//    let error = Resource::new()
//        .add_state("message", message)
//        .add_link("help", Link::new("/help"));
//
//    // todo: add vnd.error profile
//    response
//        .content_type(mimes::MediaType::Hal)
//        .send(format!("{}", error.to_json())); 
//}
//
//fn bad_request_handler(message: String, response: &mut Response) {
//    let error = Resource::new()
//        .add_state("message", message)
//        .add_link("help", Link::new("/help"));
//
//    // todo: add vnd.error profile
//    response
//        .content_type(mimes::MediaType::Hal)
//        .send(format!("{}", error.to_json())); 
//}
//
//fn error_handler<'a>(err: &mut NickelError, _request: &mut Request) -> Action {
//    let message = err.message.to_string();
//
//    if let Some(ref mut response) = err.stream {
//        match response.status() {
//            BadRequest => {
//                bad_request_handler(message, response);
//                return Halt(())
//            },
//            NotFound => {
//                not_found_handler(message, response);
//                return Halt(())
//            },
//            _ => Continue(())
//        }
//    }
//}

// todo: consider returning a Result<> instead of defaulting to Null
fn pgsql_to_hal(descs: &[Column], row: &Row) -> Resource {
    let mut hal = Resource::new();
    for desc in descs.iter() {
        let column_name = desc.name().as_slice();

        match desc.type_() {
            &Type::Varchar => {
                let value: String = row.get(column_name);
                hal = hal.add_state(column_name, value);
            },
            &Type::Int4 => {
                let value: i32 = row.get(column_name);
                hal = hal.add_state(column_name, value as i64);
            },
            &Type::Float8 => {
                let value: f64 = row.get(column_name);
                hal = hal.add_state(column_name, value);
            },
            _ => {
                println!("type is: {:?}", desc.type_());
                hal = hal.add_state(column_name, ());
            }
        }
    }

    hal
}

fn main() {

    let mut server = Nickel::new();
    server.utilize(middleware! { |request|
        let date_time = time::now();
        println!("{} logging request: {:?}", date_time.rfc3339(), request.origin.uri);
    });

    server.utilize(router! {
        get "/" => |request, response| {
            let orders = Resource::with_self("/orders")
                .add_curie("ea", "http://example.com/docs/rels/{rel}")
                .add_link("next", Link::new("/orders?page=2"))
                .add_link("ea:find", Link::new("/orders{?id}").templated(true))
                .add_link("ea:admin", Link::new("/admins/2").title("Fred"))
                .add_link("ea:admin", Link::new("/admins/5").title("Kate"))
                .add_state("currentlyProcessing", (14 as i64))
                .add_state("shippedToday", (14 as i64))
                .add_resource("ea:order",
                    Resource::with_self("/orders/123")
                        .add_link("ea:basket", Link::new("/baskets/98712"))
                        .add_link("ea:customer", Link::new("/customers/7809"))
                        .add_state("total", (30.00 as f64))
                        .add_state("currency", "USD")
                        .add_state("status", "shipped")
                )
                .add_resource("ea:order",
                    Resource::with_self("/orders/124")
                        .add_link("ea:basket", Link::new("/baskets/97213"))
                        .add_link("ea:customer", Link::new("/customers/12369"))
                        .add_state("total", (20.00 as f64))
                        .add_state("currency", "USD")
                        .add_state("status", "processing")
                );    

            let results = orders.to_json();
            response
                .content_type(mimes::MediaType::Hal)
                .send(format!("{}", results)); 
        }

        get "/orders/:order_id" => |request, response| {
            let conn = connect();

            let order_id: i32 = match FromStr::from_str(request.param("order_id")) {
                Ok(order_id) => order_id,
                Err(..) => return Err(NickelError::new(response, "Invalid order id", BadRequest))
            };

            let stmt = conn.prepare("SELECT order_id, total, currency, status
                                     FROM orders
                                     WHERE order_id = $1").unwrap();

            //let stmt = conn.prepare("SELECT row_to_json(t)
            //                        FROM (SELECT order_id, total, currency, status
            //                              FROM orders
            //                              WHERE order_id = $1
            //                        ) AS t").unwrap();

            let mut rows = match stmt.query(&[&order_id]) {
                Ok(rows) => rows,
                Err(err) => panic!("error running query: {}", err)
            };

            let row = match rows.iter().next() {
                Some(row) => row,
                None => return Err(NickelError::new(response, "No such order", NotFound))
            };

            let mut hal = pgsql_to_hal(stmt.columns(), &row);

            //let mut hal = Resource::new();
            //let descs = stmt.result_descriptions();
            //for desc in descs.iter() {
            //    println!("column name is {}", desc.name);

            //    let column_name = desc.name.as_slice();
            //    
            //    // todo: figure out how to use get_type() here
            //    match desc.ty {
            //        Type::Varchar => {
            //            let value: String = row.get(column_name);
            //            hal = hal.add_state(column_name, value);
            //        },
            //        Type::Int4 => {
            //            let value: i32 = row.get(column_name);
            //            hal = hal.add_state(column_name, value as i64);
            //        },
            //        Type::Float8 => {
            //            let value: f64 = row.get(column_name);
            //            hal = hal.add_state(column_name, value);
            //        },
            //        _ => {
            //            println!("type is: {}", desc.ty);
            //            hal = hal.add_state(column_name, ());
            //        }
            //    }
            //}

            let order_id: i32 = row.get(0);
            hal = hal.add_link("self", Link::new(format!("https://www.example.com/orders/{}", order_id).as_slice()));
            let result = hal.to_json();

            //let order = Order {
            //    order_id: row.get(0),
            //    total: row.get(1),
            //    currency: row.get(2),
            //    status: row.get(3)
            //};

            //let result = order.to_hal().to_json();
            //let result: Json = row.get(0);
            //let order = Resource::from_json(result)
            //    .add_link("self", Link::new(format!("https://www.example.com/orders/{}", order_id).as_slice()));

            response
                .content_type(mimes::MediaType::Hal)
                .send(format!("{}", result));
        }

        get "/setup" => |request, response| {
            let conn = connect();

            conn.execute("DROP TABLE IF EXISTS orders", &[]).unwrap();

            // todo: implement Numeric support
            conn.execute("CREATE TABLE orders (
                            order_id        SERIAL PRIMARY KEY,
                            total           DOUBLE PRECISION NOT NULL,
                            currency        VARCHAR NOT NULL,
                            status          VARCHAR NOT NULL
                         )", &[]).unwrap();

            conn.execute("INSERT INTO orders (order_id, total, currency, status)
                            VALUES ($1, $2, $3, $4)",
                         &[&123i32,
                           &20f64,
                           &String::from_str("USD"),
                           &String::from_str("processing")
                          ]).unwrap();

            conn.execute("INSERT INTO orders (order_id, total, currency, status)
                            VALUES ($1, $2, $3, $4)",
                         &[&124i32,
                           &30f64,
                           &String::from_str("USD"),
                           &String::from_str("shipping")
                          ]).unwrap();

            response.send("Setup complete");
        }
    });

    // issue #20178
    //let custom_handler: fn(&mut NickelError, &mut Request) -> Action = error_handler;
    //server.handle_error(custom_handler);

    server.listen("127.0.0.1:6767");
}
