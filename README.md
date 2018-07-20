Rust client library for the OpenQA WebAPI.
------------------------------------------

This uses the Hyper HTTP crate to interact with the OpenQA web API. It is
still under heavy development.

Getting Started
---------------

Probably the easiest thing to do is clone this repository and copy/edit one of
the examples to do what you want. If you wish to use it as a library in
another Rust project then see [the cargo book](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories).

Configuration
-------------

The library supports using the same configuration file as the official OpenQA
client scripts (e.g. "/etc/openqa/client.conf").

Known Problems
--------------

* Sometimes posting fails with a message suggesting the API key is wrong. This
  is probably caused by a difference between my percent encoding and Mojo's
  although I copied the algorithm Mojo uses from C, so this is a bit of a
  mystery.
* I am probably not handling errors in the most idiomatic way.
* There is some unecessary boilerplate for people who don't want to use
  Futures.
* It should implement a Serde serializer for the OpenQA's special URL encoded
  form strings (the existing crate probably won't work).
