use futures::executor::block_on;
use mockito::Server;

use super::*;

/// Sends a GET request to a mock endpoint returning `status`/`headers`/`body`, then feeds the
/// resulting response through [`ServerApi::error_from_response`].
fn error_from_mock_response(status: usize, headers: &[(&str, &str)], body: &str) -> anyhow::Error {
    let mut server = Server::new();
    let mut mock = server
        .mock("GET", "/error")
        .with_status(status)
        .with_body(body);
    for (name, value) in headers {
        mock = mock.with_header(*name, value);
    }
    mock.create();

    let url = format!("{}/error", server.url());
    block_on(async move {
        let response = http_client::Client::new_for_test()
            .get(url)
            .send()
            .await
            .unwrap();
        ServerApi::error_from_response(response).await
    })
}

/// The status carried by the [`HttpStatusError`] in `err`'s chain, if any.
fn status_in_chain(err: &anyhow::Error) -> Option<u16> {
    err.chain()
        .find_map(|cause| cause.downcast_ref::<HttpStatusError>())
        .map(|status_error| status_error.status)
}






