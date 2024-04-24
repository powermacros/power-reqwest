use power_reqwest::reqwest;
reqwest! {
    name: AliyunSmsClient,
    config: {
        ep: String,
        ak: String,
        sk: String
    },
    common: {
        unwrap_response: unwrap_response_data,
    },

    get("/api/v1/get-some") {
        name: "get_some",
        request {
            header {
                "X-Access-Token" = $access_token,
            }
            data {
                id?: string = $id,
                page?: int = $page,
            }
        }
        response {
        }
    }
}

fn main() {}
