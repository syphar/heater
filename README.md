# simple cache-warming via sitemap

This small command line tool can be used to warm CDNs or website caches, based on a sitemap.

```
USAGE:
    heater [OPTIONS] <sitemap_url>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --header <HEADER:VALUE>...    header variation

ARGS:
    <sitemap_url>    sitemap URL
```


## installation

For now, it can be simply installed globally via `cargo install heater`.

## examples

* `heater http://site/sitemap.xml`
  will read the pages in the sitemap and request all of them

* `heater http://site/sitemap.xml --header accept-language:en`
  will set the accept-language header to `en` for the requests. Any header can be set.

* `heater http://site/sitemap.xml --header accept-language:en --header accept-language:de`
  will request all the pages with **both** possible `accept-language` headers.


## debugging
setting `RUST_LOG=debug` enables some debugging output.
