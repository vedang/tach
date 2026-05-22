from fastapi import APIRouter

router = APIRouter()


@router.websocket("/ws")
def websocket_endpoint():
    return None


def unused_nested():
    return "dead"
