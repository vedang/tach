from typing import Annotated

from fastapi import Depends

from .service import USED_VALUE, helper, reexported_service as exported_service

__all__ = ["exported_by_all", "exported_service"]


def public_api(func):
    return func


DEFAULT_LABEL = "default"


class SignaturePayload:
    pass


def signature_dependency():
    return "dependency"


def signature_default_factory():
    return "default"


def used_function(
    payload: SignaturePayload,
    annotated: Annotated[SignaturePayload, Depends(signature_dependency)] = Depends(
        signature_default_factory
    ),
    label=DEFAULT_LABEL,
):
    def nested_unused():
        return "nested"

    return helper(USED_VALUE)


def unused_function():
    return "dead"


class UsedClass:
    def method(self):
        return helper(USED_VALUE)


class UnusedClass:
    def method(self):
        return "dead"


def exported_by_all():
    return "exported"


def configured_public():
    return "configured"


@public_api
def decorated_endpoint():
    return "decorated"
