extern crate http;
extern crate nickel;
extern crate hal;
extern crate serialize;

use http::headers::content_type::MediaType;
use std::io::net::ip::Ipv4Addr;
use nickel::{ Nickel, Request, Response, HttpRouter };
use hal::{ Link, Resource, ToHalState, ToHal };
use serialize::json::ToJson;

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
                    .add_state("total", (30.00 as f64).to_hal_state()) // fix precision
                    .add_state("currency", "USD".to_hal_state())
                    .add_state("status", "shipped".to_hal_state())
            )
            .add_resource("ea:order",
                Resource::with_self("/orders/124")
                    .add_link("ea:basket", Link::new("/baskets/97213"))
                    .add_link("ea:customer", Link::new("/customers/12369"))
                    .add_state("total", (20.00 as f64).to_hal_state()) // fix precision
                    .add_state("currency", "USD".to_hal_state())
                    .add_state("status", "processing".to_hal_state())
            );    

        let results = orders.to_json();
        response
            .content_type("json")
            .send(format!("{}", results)); 
    }

    fn order_handler (_request: &Request, response: &mut Response) { 
        let order = Order { total: 20.00 as f64,
        currency: String::from_str("USD"),
        status:
            String::from_str("processing")
        };

        response.origin.headers.content_type = Some(MediaType {
                type_: "application".to_string(),
                subtype: "json".to_string(),
                parameters: Vec::new()
        });

        let result = order.to_hal().to_json();
        response
            .content_type("json")
            .send(format!("{}", result)); 
    }

    let mut server = Nickel::new();
    server.get("/", index_handler);
    server.get("/orders/123", order_handler);
    server.listen(Ipv4Addr(127, 0, 0, 1), 6767);
}
