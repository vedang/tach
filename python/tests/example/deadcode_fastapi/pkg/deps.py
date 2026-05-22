from fastapi import Depends

SECURITY = object()
UNUSED_DEP_VALUE = object()


def nested_dependency(security=SECURITY):
    return security


def get_user(token=Depends(nested_dependency)):
    return token


def unused_dependency():
    return "dead"
