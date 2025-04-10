#include <jni.h>
#include "vella-sdk.h"

extern "C"
JNIEXPORT jdouble JNICALL
Java_com_vellasdk_VellaSdkModule_nativeMultiply(JNIEnv *env, jclass type, jdouble a, jdouble b) {
    return vellasdk::multiply(a, b);
}
