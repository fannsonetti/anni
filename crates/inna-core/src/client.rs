use crate::endpoints::*;
use crate::error::InnaError;
use crate::models::*;
use crate::session::StoredSession;
use chrono::NaiveDate;
use reqwest::cookie::CookieStore;
use reqwest::header::{HeaderMap, HeaderValue};
use std::sync::Arc;

pub struct InnaClient {
    http: reqwest::Client,
    jar: Arc<reqwest::cookie::Jar>,
    base_url: String,
}

impl InnaClient {
    pub fn new() -> Result<Self, InnaError> {
        let jar = Arc::new(reqwest::cookie::Jar::default());
        let http = reqwest::Client::builder()
            .cookie_provider(jar.clone())
            .timeout(std::time::Duration::from_secs(20))
            .connect_timeout(std::time::Duration::from_secs(8))
            .build()
            .map_err(|e| InnaError::Other(e.to_string()))?;

        Ok(Self {
            http,
            jar,
            base_url: INNA_BASE_URL.to_string(),
        })
    }

    pub fn load_session(&self, session: &StoredSession) {
        let url: url::Url = self.base_url.parse().unwrap();
        for cookie in &session.cookies {
            let cookie_str = format!("{}={}", cookie.name, cookie.value);
            self.jar.add_cookie_str(&cookie_str, &url);
        }
    }

    fn xsrf_token(&self) -> Option<String> {
        let url: url::Url = self.base_url.parse().unwrap();
        let cookies = self.jar.cookies(&url)?;
        let cookies_str = cookies.to_str().ok()?;

        for part in cookies_str.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("XSRF-TOKEN=") {
                return Some(decode_xsrf(val));
            }
        }
        None
    }

    fn auth_headers(&self) -> Result<HeaderMap, InnaError> {
        let token = self.xsrf_token().ok_or(InnaError::MissingXsrf)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-XSRF-TOKEN",
            HeaderValue::from_str(&token).map_err(|_| InnaError::MissingXsrf)?,
        );
        headers.insert("X-Requested-By", HeaderValue::from_static("XMLHttpRequest"));
        Ok(headers)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, InnaError> {
        let url = format!("{}{}", self.base_url, path);
        let headers = self.auth_headers()?;

        let resp = self.http.get(&url).headers(headers).send().await?;

        let status = resp.status().as_u16();
        match status {
            200 => {}
            401 | 403 => return Err(InnaError::SessionExpired),
            _ => return Err(InnaError::HttpStatus(status)),
        }

        let body = resp.text().await?;

        // Check for redirect-like HTML responses
        if body.trim_start().starts_with("<!") || body.trim_start().starts_with("<html") {
            return Err(InnaError::SessionExpired);
        }

        serde_json::from_str(&body).map_err(|_| InnaError::Parse)
    }

    pub async fn current_user(&self) -> Result<User, InnaError> {
        self.get(CURRENT_USER).await
    }

    pub async fn ping(&self) -> Result<PingResponse, InnaError> {
        self.get(PING).await
    }

    pub async fn timetable(
        &self,
        student_id: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<TimetableEntry>, InnaError> {
        let path = format!(
            "{}?staff_id=&student_id={}&moduleId=&classroom_id=&class_id=&groupId=&terms=&date_from={}&date_to={}&attendanceOverview=",
            TIMETABLE,
            student_id,
            from.format("%d.%m.%Y"),
            to.format("%d.%m.%Y"),
        );
        self.get(&path).await
    }

    pub async fn announcements(&self) -> Result<Vec<Announcement>, InnaError> {
        self.get(ANNOUNCEMENTS).await
    }

    /// Flat list used by INNA dashboard panels. Prefer [`Self::student_projects`].
    pub async fn assignments_flat(
        &self,
        assignment_type: &str,
        control: &str,
        order: &str,
    ) -> Result<Vec<Assignment>, InnaError> {
        let path = format!("{ASSIGNMENTS}?type={assignment_type}&control={control}&order={order}");
        self.get(&path).await
    }

    /// Verkefni/Próf page: assignments grouped by course, including grades.
    pub async fn student_projects(&self) -> Result<Vec<AssignmentModule>, InnaError> {
        self.get(STUDENT_PROJECTS).await
    }

    pub async fn courses(&self) -> Result<Vec<Course>, InnaError> {
        self.get(&format!("{COURSES}?termId=")).await
    }

    /// Same all-work query used by each course's Verkefni og próf page.
    pub async fn course_assignments(&self, group: &str) -> Result<Vec<Assignment>, InnaError> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("type", ""),
                ("control", "2"),
                ("order", "0"),
                ("groupId", group),
            ])
            .finish();
        self.get(&format!("{COURSE_ASSIGNMENTS}?{query}")).await
    }

    pub async fn course_homework(&self, group: &str) -> Result<Vec<Homework>, InnaError> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("groupId", group)
            .finish();
        self.get(&format!("{HOMEWORK}?{query}")).await
    }

    pub async fn homework_calendar(
        &self,
        group: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<Homework>, InnaError> {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("groupId", group),
                ("dateFrom", &from.to_string()),
                ("dateTo", &to.to_string()),
            ])
            .finish();
        self.get(&format!("{HOMEWORK_CALENDAR}?{query}")).await
    }

    pub async fn homework(&self) -> Result<Vec<Homework>, InnaError> {
        let path = format!("{}?groupId=", HOMEWORK);
        self.get(&path).await
    }

    pub async fn todo(&self, include_archived: bool) -> Result<Vec<TodoItem>, InnaError> {
        let archived = if include_archived { "1" } else { "0" };
        let path = format!("{}?withArchived={}&groupId=", TODO_LIST, archived);
        self.get(&path).await
    }
}

/// Decode URL-encoded XSRF token, matching browser decodeURIComponent behavior.
pub fn decode_xsrf(encoded: &str) -> String {
    percent_encoding::percent_decode_str(encoded)
        .decode_utf8_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn schoolwork_checks_every_course_and_retains_only_failed_sources() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut paths = Vec::new();
            for _ in 0..9 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 2048];
                while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    let read = stream.read(&mut buffer).unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&buffer[..read]);
                }
                let request = String::from_utf8(bytes).unwrap();
                let path = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_string();
                let url = url::Url::parse(&format!("http://localhost{path}")).unwrap();
                let params: std::collections::HashMap<_, _> =
                    url.query_pairs().into_owned().collect();
                let group = params.get("groupId").map(String::as_str).unwrap_or("");
                let (status, body) = match url.path() {
                    STUDENT_PROJECTS => ("200 OK", "[]"),
                    COURSES => (
                        "200 OK",
                        r#"[{"moduleName":"MATH","groupId":"g/1"},{"moduleName":"ENG","groupId":"g2"}]"#,
                    ),
                    COURSE_ASSIGNMENTS => {
                        assert_eq!(params.get("control").map(String::as_str), Some("2"));
                        assert_eq!(params.get("order").map(String::as_str), Some("0"));
                        if group == "g/1" {
                            (
                                "200 OK",
                                r#"[{"assignmentId":"1","name":"Exercises","type":"0","handInFullDate":"19.10.2026 10:00","handedIn":0}]"#,
                            )
                        } else {
                            ("200 OK", "[]")
                        }
                    }
                    HOMEWORK if group.is_empty() => (
                        "200 OK",
                        r#"[{"id":42,"text":"Read","date":"19.10.2026","moduleName":"MATH"}]"#,
                    ),
                    HOMEWORK => ("200 OK", "[]"),
                    HOMEWORK_CALENDAR if group == "g2" => ("500 Internal Server Error", "{}"),
                    HOMEWORK_CALENDAR => {
                        assert_eq!(
                            params.get("dateFrom").map(String::as_str),
                            Some("2026-07-20")
                        );
                        (
                            "200 OK",
                            r#"[{"homework":"Read","date":"19.10.2026"},{"goal":"Learning goal","date":"19.10.2026"}]"#,
                        )
                    }
                    _ => panic!("Unexpected request: {path}"),
                };
                paths.push(path);
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            paths
        });
        let mut client = InnaClient::new().unwrap();
        client.base_url = format!("http://{address}");
        client
            .jar
            .add_cookie_str("XSRF-TOKEN=test", &client.base_url.parse().unwrap());
        let saved = serde_json::from_value(serde_json::json!([
            {"text":"Stale global","id":1},
            {"text":"Stale course","id":2,"_anniSource":"course:g/1"},
            {"text":"Keep failed calendar","id":3,"_anniSource":"calendar:g2"}
        ]))
        .unwrap();
        let work = client
            .schoolwork(
                vec![],
                saved,
                &[],
                NaiveDate::from_ymd_opt(2026, 10, 18).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(work.assignments.len(), 2);
        assert_eq!(
            work.assignments[0].assignments[0].closes_str(),
            "19.10.2026 10:00"
        );
        assert_eq!(work.homework.len(), 3);
        assert!(work
            .homework
            .iter()
            .any(|h| h.raw["text"] == "Keep failed calendar"));
        assert!(!work
            .homework
            .iter()
            .any(|h| h.raw["text"] == "Stale course"));
        assert_eq!(work.warnings.len(), 1);
        assert_eq!(work.warnings[0].0, "homework");
        assert_eq!(server.join().unwrap().len(), 9);
    }

    #[test]
    fn test_xsrf_decode_plain() {
        assert_eq!(decode_xsrf("abc123"), "abc123");
    }

    #[test]
    fn test_xsrf_decode_encoded() {
        assert_eq!(decode_xsrf("abc%2F123%3D%3D"), "abc/123==");
    }

    #[test]
    fn test_xsrf_decode_plus() {
        // percent_decode does not decode '+' as space (matching decodeURIComponent)
        assert_eq!(decode_xsrf("a+b"), "a+b");
    }
}
