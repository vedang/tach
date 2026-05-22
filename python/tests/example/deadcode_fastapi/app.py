from fastapi import FastAPI

from pkg.admin import router as admin_router
from pkg.api import router as api_router

app = FastAPI()
app.include_router(api_router)


@app.get("/health")
def health_check():
    return {"ok": True}


@app.api_route("/status", methods=["GET"])
def status_check():
    return {"ok": True}


def unused_local():
    return admin_router
