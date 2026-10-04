from aura_sdk.router import route_cognitive_demand


def test_route_deep_research():
    assert route_cognitive_demand("synthesize a market analysis") == {"agent": "deep-research-preview"}

def test_route_pro():
    assert route_cognitive_demand("plan the system architecture") == {"model": "gemini-3.1-pro"}
    assert route_cognitive_demand("a" * 2001) == {"model": "gemini-3.1-pro"}

def test_route_flash_lite():
    assert route_cognitive_demand("please validate this log file") == {"model": "gemini-3.5-flash-lite"}

def test_route_flash():
    assert route_cognitive_demand("write a simple python script to do this") == {"model": "gemini-3.8-flash"}
