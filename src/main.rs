extern crate http;
extern crate nickel;
extern crate hal;
extern crate serialize;
extern crate postgres;

use std::io::net::ip::Ipv4Addr;
use nickel::{Nickel, Request, Response, HttpRouter, Continue, Halt, MiddlewareResult};
use nickel::{NickelError, ErrorWithStatusCode, mimes};
use hal::{Link, Resource, ToHalState, ToHal};
use serialize::json::ToJson;
use postgres::{Connection, NoSsl};
use std::os;
use http::status::NotFound;
use http::status::BadRequest;

struct Order {
    order_id: i32,
    total: f64,
    currency: String,
    status: String
}

impl ToHal for Order {
    fn to_hal(&self) -> Resource {
        Resource::with_self(format!("https://www.example.com/orders/{}", self.order_id).as_slice())
            .add_state("total", self.total.to_hal_state())
            .add_state("currency", self.currency.to_hal_state())
            .add_state("status", self.status.to_hal_state())
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
        .add_state("message", message.to_hal_state())
        .add_link("help", Link::new("/help"));

    // todo: add vnd.error profile
    response
        .status_code(NotFound)
        .content_type(mimes::Hal)
        .send(format!("{}", error.to_json())); 
}

fn bad_request_handler(message: String, response: &mut Response) {
    let error = Resource::new()
        .add_state("message", message.to_hal_state())
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

fn main() {

    fn index_handler (_request: &Request, response: &mut Response) { 
        let orders = Resource::with_self("/orders")
            .add_curie("ea", "http://example.com/docs/rels/{rel}")
            .add_link("next", Link::new("/orders?page=2"))
            .add_link("ea:find", Link::new("/orders{?id}").templated(true))
            .add_link("ea:admin", Link::new("/admins/2").title("Fred"))
            .add_link("ea:admin", Link::new("/admins/5").title("Kate"))
            .add_state("currentlyProcessing", (14 as int).to_hal_state())
            .add_state("shippedToday", (14 as int).to_hal_state())
            .add_resource("ea:order",
                Resource::with_self("/orders/123")
                    .add_link("ea:basket", Link::new("/baskets/98712"))
                    .add_link("ea:customer", Link::new("/customers/7809"))
                    .add_state("total", (30.00 as f64).to_hal_state())
                    .add_state("currency", "USD".to_hal_state())
                    .add_state("status", "shipped".to_hal_state())
            )
            .add_resource("ea:order",
                Resource::with_self("/orders/124")
                    .add_link("ea:basket", Link::new("/baskets/97213"))
                    .add_link("ea:customer", Link::new("/customers/12369"))
                    .add_state("total", (20.00 as f64).to_hal_state())
                    .add_state("currency", "USD".to_hal_state())
                    .add_state("status", "processing".to_hal_state())
            );    

        let results = orders.to_json();
        response
            .content_type(mimes::Hal)
            .send(format!("{}", results)); 
    }

    fn order_handler (request: &Request, response: &mut Response) -> MiddlewareResult { 
        let conn = connect();

        let order_id: i32 = match from_str(request.param("order_id")) {
            Some(order_id) => order_id,
            None => return Err(NickelError::new("Invalid order id", ErrorWithStatusCode(BadRequest)))
        };

        let stmt = conn.prepare("SELECT order_id, total, currency, status
                                 FROM orders
                                 WHERE order_id = $1").unwrap();

        let mut rows = match stmt.query(&[&order_id]) {
            Ok(rows) => rows,
            Err(err) => panic!("error running query: {}", err)
        };

        let row = match rows.next() {
            Some(row) => row,
            None => return Err(NickelError::new("No such order", ErrorWithStatusCode(NotFound)))
        };

        let order = Order {
            order_id: row.get(0),
            total: row.get(1),
            currency: row.get(2),
            status: row.get(3)
        };

        let result = order.to_hal().to_json();
        response
            .content_type(mimes::Hal)
            .send(format!("{}", result)); 

        // todo: find out why i have to halt here
        Ok(Halt)
    }

    fn setup_handler (_request: &Request, response: &mut Response) { 
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

    let mut server = Nickel::new();

    let mut router = Nickel::router();

    router.get("/", index_handler);
    router.get("/orders/:order_id", order_handler);
    router.get("/setup", setup_handler);

    server.utilize(router);
    server.handle_error(error_handler);
    server.listen(Ipv4Addr(127, 0, 0, 1), 6767);
}
