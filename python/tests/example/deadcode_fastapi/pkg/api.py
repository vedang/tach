from typing import Annotated

from fastapi import APIRouter, Depends

from .deps import get_user, nested_dependency
from .models import ItemIn, ItemOut, UserContext
from .nested import router as nested_router

router = APIRouter()
router.include_router(nested_router)


@router.get(
    "/items/{item_id}",
    response_model=list[ItemOut],
    dependencies=[Depends(nested_dependency)],
)
def read_item(
    item: ItemIn,
    user: Annotated[UserContext, Depends(get_user)],
):
    return []


@router.head("/items")
def head_items():
    return None


def unused_api_handler():
    return "dead"
