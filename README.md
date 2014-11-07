# hal-rs-demo

A demonstration of how to use the hal-rs library in a web server.

[![Build Status](https://travis-ci.org/hjr3/hal-rs-demo.svg)](https://travis-ci.org/hjr3/hal-rs-demo)

## Build Instructions

```
$ git clone https://github.com/hjr3/hal-rs-demo
$ cd hal-rs-demo
$ cargo build
$ cargo run
```

The demo requires a postgres database server. The postgres database server
credentials can be customized using the following environment variable:

   * DBHOST
   * DBPORT
   * DBUSER
   * DBPASS
   * DBNAME

## Setup

There is fixture data that will setup the database and allow the requests to
work. Please make sure the database user is allowed to create and drop tables.

The setup assumes the database is already created.

Browse to http://localhost:6767/setup and you should see "Setup complete".

## Examples

The two working examples are:

   * `/` - returns a Hal collection that was manually constructed
   * `/orders/:order_id` returns a Hal object via the `ToHal` trait being implemented on an `Order` struct
