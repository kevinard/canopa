/* Isolated acceptance fixture only. Never install in the host PAM configuration. */
#include <security/pam_modules.h>
#include <security/pam_appl.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

PAM_EXTERN int pam_sm_authenticate(pam_handle_t *handle, int flags, int argc, const char **argv) {
    (void)flags; (void)argc; (void)argv;
    const struct pam_conv *conversation = NULL;
    int status = pam_get_item(handle, PAM_CONV, (const void **)&conversation);
    if (status != PAM_SUCCESS || !conversation) return PAM_SYSTEM_ERR;
    const struct pam_message message = { PAM_PROMPT_ECHO_OFF, "Fixture password:" };
    const struct pam_message *messages[] = { &message };
    struct pam_response *reply = NULL;
    status = conversation->conv(1, messages, &reply, conversation->appdata_ptr);
    if (status != PAM_SUCCESS || !reply || !reply->resp) return PAM_AUTH_ERR;
    int accepted = strcmp(reply->resp, "canopa-fixture-password") == 0;
    explicit_bzero(reply->resp, strlen(reply->resp));
    free(reply->resp); free(reply);
    fprintf(stderr, "fixture auth accepted=%d\n", accepted); fflush(stderr);
    return accepted ? PAM_SUCCESS : PAM_AUTH_ERR;
}
PAM_EXTERN int pam_sm_setcred(pam_handle_t *handle, int flags, int argc, const char **argv) {
    (void)handle; (void)flags; (void)argc; (void)argv; return PAM_SUCCESS;
}
PAM_EXTERN int pam_sm_acct_mgmt(pam_handle_t *handle, int flags, int argc, const char **argv) {
    (void)handle; (void)flags;
    return argc && access(argv[0], F_OK) == 0 ? PAM_ACCT_EXPIRED : PAM_SUCCESS;
}
