#[cfg(test)]
mod tests {
    use crate::cfs::configuration::mesa::utils::create;
    use super::*;

    use inline_c::assert_c;

    #[test]
    fn test_result() {
        (assert_c! {
            #include <libmesa.h>
            #include <stdio.h>
            int main() {
                const char* token   = "token";
                const char* url     = "url";
                const char* cert    = "cert";
                const char* name    = "groupname";
                const char* result = C_mesa_tf_hsmgroup_create(token, url, cert, name);
                printf("Result:%s\n", result);
                int x = 1;
                int y = 2;

                return x + y;
            }
        })
            .failure()
            .code(3);
    }
    #[test]
    fn test_stdout() {
        (assert_c! {
        #include <stdio.h>

        int main() {
            printf("Hello, World!");

            return 0;
        }
    })
            .success()
            .stdout("Hello, World!");
    }
    #[test]
    fn test_create() {
        let url = "url";
        let token = "token";
        let cert = "cert".as_bytes();
        let _group_name = "group_name".to_string();
        let group_name = Option::from(&_group_name);
        assert_eq!(csm::hsmgroup::create(token, url, cert, group_name),format!("{token}{url}"));
    }
    // #[test]
    // fn test_rust_produce_string() {
    //     let my_string = "this is a stupid string".to_string();
    //     let result = create(my_string);
    //     assert_eq!(result, "Hello, I am a rust string, and the following comes from C: this is a stupid string")
    // }
}