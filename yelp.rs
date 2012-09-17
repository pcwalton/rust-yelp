// Rust bindings to the Yelp API version 2.0.

extern mod http_client;
extern mod oauth;
extern mod std;

use mod std::json;
use mod std::net::url;
use mod std::time;
use core::future::Future;
use core::rand::Rng;
use core::send_map::linear::LinearMap;
use core::to_str::ToStr;
use std::json::{Dict, Json, List, String};
use std::net::url::Url;

use oauth::{Consumer, HmacSha1, Token};

type QueryParams = LinearMap<~str,~str>;

// FIXME: Linker bustage workaround. This is really really terrible.
#[link_args="-lnss3"]
#[nolink]
extern {
}

fn slurp(+url: Url) -> Future<~str> {
    do future::spawn {
        let result = @dvec::DVec();
        let request = http_client::uv_http_request(url);
        do request.begin |event, copy result| {
            match event {
                http_client::Error(e) => {
                    debug!("got error: %?", e);
                }
                http_client::Status(*) => {},
                http_client::Payload(data_ref) => {
                    // FIXME: Why is data_ref a ~mut Option? I hate option dances!
                    result.push_all(data_ref.get())
                }
            }
        }
        let result = str::from_bytes(dvec::unwrap(*result));
        debug!("done: %s", result);
        result
    }
}

pub mod search {
    const URL: &static/str = "http://api.yelp.com/v2/search";

    pub enum QueryLocation {
        NeighborhoodAddressCity(&str)
    }

    impl QueryLocation {
        fn add_to(&self, params: &mut QueryParams) {
            match *self {
                NeighborhoodAddressCity(query) => {
                    params.insert(~"location", query.to_str());
                }
            }
        }
    }

    pub struct Options {
        term: Option<&str>,
        location: QueryLocation
    }

    impl Options {
        fn add_to(&self, params: &mut QueryParams) {
            do self.term.iter |term| {
                params.insert(~"term", term.to_str());
            }
            (&self.location).add_to(params);
        }
    }

    pub fn defaults(location: QueryLocation/&a) -> Options/&a {
        Options {
            term: None,
            location: location
        }
    }

    // Responses

    pub struct Business {
        name: ~str
    }

    pub struct Result {
        businesses: ~[Business]
    }

    mod Result {
        fn from_json(json: &Json) -> Result {
            // FIXME: std::json could use some helper methods for this stuff.
            let businesses = dvec::DVec();
            match *json {
                Dict(dict) => {
                    match dict.get(~"businesses") {
                        List(list) => {
                            for list.each |business| {
                                match business {
                                    Dict(dict) => {
                                        let name;
                                        match dict.get(~"name") {
                                            String(s) => name = copy *s,
                                            _ => fail
                                        }
                                        businesses.push(Business { name: move name });
                                    }
                                    _ => fail
                                }
                            }
                        }
                        _ => fail
                    }
                }
                _ => fail
            }

            Result {
                businesses: dvec::unwrap(businesses)
            }
        }
    }

    pub fn search(rng: @Rng, consumer: &Consumer, token: &Token, options: &Options) ->
                  Future<Result> {
        let mut params = LinearMap();

        // FIXME: Auto-ref would be very useful here.
        // FIXME: The OAuth library should be able to fill in some of this stuff.
        (&mut params).insert(~"oauth_consumer_key", consumer.key.to_str());
        (&mut params).insert(~"oauth_token", token.key.to_str());
        (&mut params).insert(~"oauth_signature_method", HmacSha1.to_str());
        (&mut params).insert(~"oauth_timestamp", time::get_time().sec.to_str());
        (&mut params).insert(~"oauth_nonce", rng.next().to_str());

        options.add_to(&mut params);

        let mut url = url::from_str(URL).get();
        let signature = oauth::Request {
                method: "GET",
                url: &url,
                parameters: &params
            }.sign(HmacSha1, consumer, Some(token));

        (&mut params).insert(~"oauth_signature", signature);

        // Turn the parameters into a query.
        // FIXME: This should really be in the standard library.
        let mut query = dvec::DVec();
        for (&params).each_ref |key, value| {
            query.push((copy *key, copy *value));
        }

        let params = dvec::unwrap(query);    // FIXME: These should be methods.
        url.query = move params;

        debug!("sending request: %s", url::to_str(url));
        do future::spawn |move url| {
            let text = slurp(url);

            // FIXME: This is a particularly ugly line. We need method-ification and auto-ref.
            let json = result::unwrap(json::from_str(*(&text).get_ref()));

            Result::from_json(&json)
        }
    }
}

fn main(args: ~[~str]) {
    use search::Options;

    // FIXME: This is beyond awful! Why doesn't assignability work?
    let consumer_key: &str = str::view(args[1], 0, args[1].len());
    let consumer_secret: &str = str::view(args[2], 0, args[2].len());
    let token_key: &str = str::view(args[3], 0, args[3].len());
    let token_secret: &str = str::view(args[4], 0, args[4].len());

    let consumer = Consumer { key: consumer_key, secret: consumer_secret };
    let token = Token { key: token_key, secret: token_secret };
    let options = Options {
        term: Some("restaurants"),
        location: search::NeighborhoodAddressCity("94306")
    };

    io::println("Sending request...");
    let result = search::search(rand::xorshift(), &consumer, &token, &options);
    io::println("Result complete!");
    io::println(fmt!("%?", *(&result).get_ref()));
}

