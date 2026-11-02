/* Test-only non-resident U2F enrollment. Does not change PINs or resident keys. */
#include <fido.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int encoded(const unsigned char *data, size_t length) {
    if (!data || !length || length > 4096) return -1;
    unsigned char output[5465];
    int size = EVP_EncodeBlock(output, data, (int)length);
    return size > 0 && fwrite(output, 1, (size_t)size, stdout) == (size_t)size ? 0 : -1;
}
int main(int argc, char **argv) {
    if (argc != 4 || strpbrk(argv[3], ":,\r\n")) return 2;
    int status = FIDO_ERR_INTERNAL, result = 1;
    unsigned char challenge[32], user_id[32];
    fido_init(0);
    fido_dev_t *device = fido_dev_new();
    fido_cred_t *credential = fido_cred_new();
    if (!device || !credential || RAND_bytes(challenge, sizeof(challenge)) != 1 ||
        RAND_bytes(user_id, sizeof(user_id)) != 1) goto done;
    if ((status = fido_dev_open(device, argv[1])) != FIDO_OK) goto done;
    fido_dev_force_u2f(device);
    if ((status = fido_cred_set_type(credential, COSE_ES256)) != FIDO_OK ||
        (status = fido_cred_set_clientdata_hash(credential, challenge, sizeof(challenge))) != FIDO_OK ||
        (status = fido_cred_set_rp(credential, argv[2], argv[2])) != FIDO_OK ||
        (status = fido_cred_set_user(credential, user_id, sizeof(user_id), argv[3], argv[3], NULL)) != FIDO_OK) goto done;
    fprintf(stderr, "Touch the key to create a test-only U2F credential.\n");
    if ((status = fido_dev_make_cred(device, credential, NULL)) != FIDO_OK) goto done;
    status = fido_cred_x5c_ptr(credential) ? fido_cred_verify(credential) : fido_cred_verify_self(credential);
    if (status != FIDO_OK) goto done;
    /* pam-u2f's native mapping: username:key handle,raw public key,algorithm,options. */
    if (printf("%s:", argv[3]) < 0 || encoded(fido_cred_id_ptr(credential), fido_cred_id_len(credential)) ||
        putchar(',') == EOF || encoded(fido_cred_pubkey_ptr(credential), fido_cred_pubkey_len(credential)) ||
        printf(",es256,+presence\n") < 0 || fflush(stdout)) goto done;
    result = 0;
done:
    if (result) fprintf(stderr, "U2F test enrollment failed: %s (%d)\n", fido_strerr(status), status);
    if (device) fido_dev_close(device);
    fido_dev_free(&device);
    fido_cred_free(&credential);
    return result;
}
