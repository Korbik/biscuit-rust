//#[cfg(test)]
#[cfg(feature = "capi")]
mod capi {
    use inline_c::assert_c;

    #[test]
    fn build() {
        (assert_c! {
            #include <stdio.h>
            #include <string.h>
            #include "biscuit_auth.h"

            int main() {
                char *seed = "abcdefghabcdefghabcdefghabcdefgh";

                KeyPair * root_kp = key_pair_new((const uint8_t *) seed, strlen(seed));
                printf("key_pair creation error? %s\n", error_message());
                PublicKey* root = key_pair_public(root_kp);

                BiscuitBuilder* b = biscuit_builder(root_kp);
                printf("builder creation error? %s\n", error_message());
                biscuit_builder_add_authority_fact(b, "right(#authority, \"file1\", #read)");

                printf("builder add authority error? %s\n", error_message());

                Biscuit * biscuit = biscuit_builder_build(b, (const uint8_t * ) seed, strlen(seed));
                printf("Hello, World!\n");
                printf("biscuit creation error? %s\n", error_message());

                Verifier * verifier = biscuit_verify(biscuit, root);
                printf("verifier creation error? %s\n", error_message());
                verifier_add_caveat(verifier, "*right(#abcd) <- right(#efgh)");
                printf("verifier add caveat error? %s\n", error_message());
                char* world_print = verifier_print(verifier);
                printf("verifier world:\n%s\n", world_print);
                string_free(world_print);
                if(!verifier_verify(verifier)) {
                    printf("verifier error: %s\n", error_message());
                } else {
                    printf("verifier succeeded\n");
                }

                uint64_t sz = biscuit_serialized_size(biscuit);
                printf("serialized size: %ld\n", sz);
                uint8_t * buffer = malloc(sz);
                uint64_t written = biscuit_serialize(biscuit, buffer);
                printf("wrote %ld bytes\n", written);

                free(buffer);
                verifier_free(verifier);
                biscuit_free(biscuit);
                public_key_free(root);
                key_pair_free(root_kp);

                return 0;
            }
        })
        .success()
        .stdout("Hello world");
    }
}
