use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    api_token: Secret<String>,
    secret_token: Secret<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct EmailInformation<'a> {
    email: &'a str,
    name: Option<&'a str>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: EmailInformation<'a>,
    to: Vec<EmailInformation<'a>>,
    subject: &'a str,
    #[serde(rename = "HTMLPart")]
    html_part: &'a str,
    text_part: &'a str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequestBody<'a> {
    messages: Vec<SendEmailRequest<'a>>,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        api_token: Secret<String>,
        secret_token: Secret<String>,
    ) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            sender,
            api_token,
            secret_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/send", self.base_url);
        let request_body_inner = SendEmailRequest {
            from: EmailInformation {
                email: self.sender.as_ref(),
                name: None,
            },
            to: vec![EmailInformation {
                email: recipient.as_ref(),
                name: None,
            }],
            subject,
            html_part: html_content,
            text_part: text_content,
        };
        let request_body = SendEmailRequestBody {
            messages: vec![request_body_inner],
        };
        self.http_client
            .post(&url)
            .basic_auth(
                self.api_token.expose_secret(),
                Some(self.secret_token.expose_secret()),
            )
            .json(&request_body)
            .send()
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            // Try to parse the body as a JSON value
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                let message_body = &body.get("Messages").unwrap()[0];
                body.get("Messages").is_some()
                    && message_body.get("From").is_some()
                    && message_body.get("From").unwrap().get("Email").is_some()
                    && message_body.get("To").is_some()
                    && message_body.get("To").unwrap()[0].get("Email").is_some()
                    && message_body.get("Subject").is_some()
                    && message_body.get("HTMLPart").is_some()
                    && message_body.get("TextPart").is_some()
            } else {
                false
            }
        }
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        // Arrange
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let email_client = EmailClient::new(
            mock_server.uri(),
            sender,
            Secret::new(Faker.fake()),
            Secret::new(Faker.fake()),
        );

        Mock::given(header_exists("Authorization"))
            .and(header("Content-Type", "application/json"))
            .and(path("/send"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        // Act
        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;

        // Assert
    }
}
