# hal-rs-demo

A demonstration of how to use the hal-rs library in a web server.

[![Build Status](https://travis-ci.org/hjr3/hal-rs-demo.svg)](https://travis-ci.org/hjr3/hal-rs-demo)

## Instructions

```
$ git clone https://github.com/hjr3/hal-rs-demo
$ cd hal-rs-demo
$ cargo build
$ cargo run
```

Browse to http://localhost:6767/

## Examples

The two working examples are:

   * `/` - returns a Hal collection that was manually constructed
   * `/orders/123` returns a Hal object via the `ToHal` trait being implemented on an `Order` struct
