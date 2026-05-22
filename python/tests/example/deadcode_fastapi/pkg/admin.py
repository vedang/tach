from fastapi import APIRouter

router = APIRouter()


@router.get("/admin")
def admin_handler():
    return "dead"


def unused_admin_helper():
    return "dead"
