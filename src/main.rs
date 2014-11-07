extern crate http;
extern crate nickel;
extern crate hal;
extern crate serialize;
extern crate postgres;

use std::io::net::ip::Ipv4Addr;
use nickel::{ Nickel, Request, Response, HttpRouter };
use hal::{ Link, Resource, ToHalState, ToHal };
use serialize::json::ToJson;
use postgres::{Connection, NoSsl};
use std::os;

struct Order {
    total: f64,
    currency: String,
    status: String
}

impl ToHal for Order {
    fn to_hal(&self) -> Resource {
        Resource::with_self("https://www.example.com/orders/1")
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
            .content_type("json")
            .send(format!("{}", results)); 
    }

    fn order_handler (request: &Request, response: &mut Response) { 
        let conn = connect();

        let order_id: i32 = from_str(request.param("order_id")).unwrap();

        let stmt = conn.prepare("SELECT total, currency, status
                                 FROM orders
                                 WHERE order_id = $1").unwrap();


        let mut rows = match stmt.query(&[&order_id]) {
            Ok(rows) => rows,
            Err(err) => panic!("error running query: {}", err)
        };

        let row = rows.next().unwrap();

        let order = Order {
            total: row.get(0),
            currency: row.get(1),
            status: row.get(2)
        };

        let result = order.to_hal().to_json();
        response
            .content_type("json")
            .send(format!("{}", result)); 
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
    server.get("/", index_handler);
    server.get("/orders/:order_id", order_handler);
    server.get("/setup", setup_handler);
    server.listen(Ipv4Addr(127, 0, 0, 1), 6767);
}
