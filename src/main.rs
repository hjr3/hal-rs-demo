#![feature(phase)]

extern crate http;
extern crate nickel;
extern crate hal;
extern crate serialize;
extern crate postgres;
extern crate time;
#[phase(plugin)] extern crate nickel_macros;

use std::io::net::ip::Ipv4Addr;
use nickel::{Nickel, Request, Response, HttpRouter, Continue, Halt, MiddlewareResult};
use nickel::{NickelError, ErrorWithStatusCode, mimes};
use hal::{Link, Resource, ToHal};
use serialize::json::{ToJson, Json};
use postgres::{Connection, NoSsl, Row, Rows, Statement, ToSql, ResultDescription};
use postgres::types::{Type};
use std::os;
use http::status::{NotFound, BadRequest};

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
    match os::getenv(key) {
        Some(val) => val,
        None => String::from_str(default)
    }
}

fn connect() -> Connection {
    let host = get_option("DBHOST", "localhost");
    let port = get_option("DBPORT", "15432");
    let user = get_option("DBUSER", "myapp");
    let password = get_option("DBPASS", "dbpass");
    let dbname = get_option("DBNAME", "myapp");

    let dsn = format!("postgres://{}:{}@{}:{}/{}", user, password, host, port, dbname);
    Connection::connect(dsn.as_slice(), &NoSsl).unwrap()
}

fn not_found_handler(message: String, response: &mut Response) {
    let error = Resource::new()
        .add_state("message", message)
        .add_link("help", Link::new("/help"));

    // todo: add vnd.error profile
    response
        .status_code(NotFound)
        .content_type(mimes::Hal)
        .send(format!("{}", error.to_json())); 
}

fn bad_request_handler(message: String, response: &mut Response) {
    let error = Resource::new()
        .add_state("message", message)
        .add_link("help", Link::new("/help"));

    // todo: add vnd.error profile
    response
        .status_code(BadRequest)
        .content_type(mimes::Hal)
        .send(format!("{}", error.to_json())); 
}

fn error_handler(err: &NickelError, _request: &Request, response: &mut Response) -> MiddlewareResult {
    let message = err.message.to_string();

    match err.kind {
        ErrorWithStatusCode(BadRequest) => {
            bad_request_handler(message, response);
            Ok(Halt)
        },
        ErrorWithStatusCode(NotFound) => {
            not_found_handler(message, response);
            Ok(Halt)
        },
        _ => Ok(Continue)
    }
}

fn logger(request: &Request, _response: &mut Response) -> MiddlewareResult {
    let date_time = time::now();

    println!("{} {}", date_time.rfc3339(), request.origin.request_uri);

    Ok(Continue)
}

// todo: consider returning a Result<> instead of defaulting to Null
fn pgsql_to_hal(descs: &[ResultDescription], row: &Row) -> Resource {
    let mut hal = Resource::new();
    for desc in descs.iter() {
        let column_name = desc.name.as_slice();

        match desc.ty {
            Type::Varchar => {
                let value: String = row.get(column_name);
                hal = hal.add_state(column_name, value);
            },
            Type::Int4 => {
                let value: i32 = row.get(column_name);
                hal = hal.add_state(column_name, value as i64);
            },
            Type::Float8 => {
                let value: f64 = row.get(column_name);
                hal = hal.add_state(column_name, value);
            },
            _ => {
                println!("type is: {}", desc.ty);
                hal = hal.add_state(column_name, ());
            }
        }
    }

    hal
}

fn main() {

    let mut server = Nickel::new();
    server.utilize(logger);

    server.utilize(router! {
        get "/" => |request, response| {
            let orders = Resource::with_self("/orders")
                .add_curie("ea", "http://example.com/docs/rels/{rel}")
                .add_link("next", Link::new("/orders?page=2"))
                .add_link("ea:find", Link::new("/orders{?id}").templated(true))
                .add_link("ea:admin", Link::new("/admins/2").title("Fred"))
                .add_link("ea:admin", Link::new("/admins/5").title("Kate"))
                .add_state("currentlyProcessing", (14 as int))
                .add_state("shippedToday", (14 as int))
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
                .content_type(mimes::Hal)
                .send(format!("{}", results)); 
        }

        get "/orders/:order_id" => |request, response| {
            let conn = connect();

            let order_id: i32 = match from_str(request.param("order_id")) {
                Some(order_id) => order_id,
                None => return Err(NickelError::new("Invalid order id", ErrorWithStatusCode(BadRequest)))
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

            let row = match rows.next() {
                Some(row) => row,
                None => return Err(NickelError::new("No such order", ErrorWithStatusCode(NotFound)))
            };

            let mut hal = pgsql_to_hal(stmt.result_descriptions(), &row);

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
                .content_type(mimes::Hal)
                .send(format!("{}", result));

            Ok(Halt)
        }

        get "/setup" => |request, response| {
            let conn = connect();

            conn.execute("DROP TABLE IF EXISTS orders", []).unwrap();

            // todo: implement Numeric support
            conn.execute("CREATE TABLE orders (
                            order_id        SERIAL PRIMARY KEY,
                            total           DOUBLE PRECISION NOT NULL,
                            currency        VARCHAR NOT NULL,
                            status          VARCHAR NOT NULL
                         )", []).unwrap();

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
    server.handle_error(error_handler);
    server.listen(Ipv4Addr(127, 0, 0, 1), 6767);
}
