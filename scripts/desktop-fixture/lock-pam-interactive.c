/* Isolated test module. Never install this fixture as a host PAM policy. */
#include <security/pam_modules.h>
#include <security/pam_appl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
static const char *root;
static void mark(const char *event) {
    char path[4096];snprintf(path,sizeof(path),"%s/auth-events",root);
    FILE *file=fopen(path,"a");if(file){fprintf(file,"%ld %ld %s\n",(long)getpid(),(long)getppid(),event);fclose(file);}
}
static int converse(pam_handle_t *handle,int style,const char *text,const char *expected) {
    const struct pam_conv *conversation=NULL;
    if(pam_get_item(handle,PAM_CONV,(const void**)&conversation)!=PAM_SUCCESS || !conversation)return PAM_SYSTEM_ERR;
    struct pam_message message={style,text};const struct pam_message *messages[]={&message};struct pam_response *reply=NULL;
    mark(text);
    int status=conversation->conv(1,messages,&reply,conversation->appdata_ptr);
    if(status!=PAM_SUCCESS || !reply)return PAM_AUTH_ERR;
    int accepted=!expected || (reply->resp && strcmp(reply->resp,expected)==0);
    if(expected)mark(accepted?"response accepted":"response rejected");
    if(reply->resp){explicit_bzero(reply->resp,strlen(reply->resp));free(reply->resp);}free(reply);
    return accepted?PAM_SUCCESS:PAM_AUTH_ERR;
}
PAM_EXTERN int pam_sm_authenticate(pam_handle_t *handle,int flags,int argc,const char **argv) {
    (void)flags;if(argc<2)return PAM_SYSTEM_ERR;root=argv[1];mark("started");
    const char *mode=argv[0];int result=PAM_SUCCESS;
    if(strcmp(mode,"fingerprint")!=0) {
        result=converse(handle,PAM_PROMPT_ECHO_OFF,"Password:","canopa-fixture-password");if(result!=PAM_SUCCESS)return result;
    }
    if(strcmp(mode,"otp")==0) {
        return converse(handle,PAM_PROMPT_ECHO_ON,"One-time code:","123456");
    }
    if(strcmp(mode,"token")==0 || strcmp(mode,"fingerprint")==0) {
        result=converse(handle,PAM_TEXT_INFO,strcmp(mode,"token")==0?"Touch your security key":"Scan your fingerprint",NULL);
        if(result!=PAM_SUCCESS)return result;
        char path[4096];snprintf(path,sizeof(path),"%s/touch",root);
        for(int attempt=0;attempt<300;attempt++){if(access(path,F_OK)==0){mark("device accepted");return PAM_SUCCESS;}usleep(100000);}
        return PAM_AUTH_ERR;
    }
    return PAM_SUCCESS;
}
PAM_EXTERN int pam_sm_setcred(pam_handle_t *handle,int flags,int argc,const char **argv){(void)handle;(void)flags;(void)argc;(void)argv;return PAM_SUCCESS;}
PAM_EXTERN int pam_sm_acct_mgmt(pam_handle_t *handle,int flags,int argc,const char **argv){(void)handle;(void)flags;(void)argc;(void)argv;return PAM_SUCCESS;}
