// Our class for handling NSServices messages.
@interface LeantermServicesProvider : NSObject
@end

// Functions implemented in Rust.
id leanterm_services_provider_custom_url_scheme();
void leanterm_app_open_urls(id app, id urls);
