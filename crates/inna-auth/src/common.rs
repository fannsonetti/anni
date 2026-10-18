use inna_core::endpoints::INNA_BASE_URL;

pub const LOGIN_URL: &str = INNA_BASE_URL;
pub const CHECK_URL: &str = "/api/UserData/GetLoggedInUser";

pub fn full_check_url() -> String {
    format!("{INNA_BASE_URL}{CHECK_URL}")
}
