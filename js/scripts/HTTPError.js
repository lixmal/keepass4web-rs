export default class HTTPError extends Error {
    constructor(response, message, data) {
        super(response)
        this.name = "HTTPError"
        this.msg = message
        this.status = response.status
        this.data = data
    }

    toString() {
        return this.msg
    }
}