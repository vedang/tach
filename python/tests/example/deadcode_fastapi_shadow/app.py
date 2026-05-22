def FastAPI():
    return object()


def APIRouter():
    return object()


app = FastAPI()
router = APIRouter()
app.include_router(router)


@app.get("/shadow")
def shadow_app_route():
    return "dead"


@router.get("/shadow-router")
def shadow_router_route():
    return "dead"
