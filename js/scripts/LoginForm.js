import React from 'react'
import Classnames from 'classnames'


export default class LoginForm extends React.Component {
    constructor(props) {
        super(props)
        this.handleLogin = this.handleLogin.bind(this)
        this.abortRequests = this.abortRequests.bind(this)
        this.state = {
            error: null,
            mask: false
        }
    }

    transformRefs(tRefs) {
        let refs = {}
        for (let property in tRefs) {
            if (tRefs.hasOwnProperty(property)) {
                refs[property] = tRefs[property].value
            }
        }

        return refs
    }

    classes() {
        return Classnames({
            'kp-login': true,
            'loading-mask': this.state.mask,
        })
    }

    abortRequests() {
        if (this.serverRequest)
            this.serverRequest.abort()

        if (this.authRequest)
            this.authRequest.abort()
    }

    handleLogin(event) {
        event.preventDefault()
        if (this.state.mask)
            return

        this.abortRequests()

        this.setState({
            error: null,
            mask: true
        })
        this.serverRequest = KeePass4Web.ajax(this.url, {
            success: function (data) {
                if (data && data.data) {
                    KeePass4Web.setCSRFToken(data.data.csrf_token)
                    KeePass4Web.setSettings(data.data.settings)
                }
            }.bind(this),
            data: this.transformRefs(this.refs),
            error: function (r, s, e) {
                var errmsg = s

                // error code sent by server
                if (s == 'error' && r.responseJSON) {
                    errmsg = r.responseJSON.message
                }

                this.setState({
                    error: errmsg,
                    mask: false
                })
            }.bind(this),
            complete: function () {
                this.serverRequest = null
                // check which authentication is required next
                // preceding auth could've expired
                this.authRequest = KeePass4Web.checkAuth.call(this)
            }.bind(this)
        })

    }

    componentDidMount() {
        // default 10 minutes
        this.timerId = setInterval(function () {
            // don't interfere with ongoing login process
            if (this.serverRequest) return

            // keep state for "session expired" etc messages
            this.authRequest = KeePass4Web.checkAuth.call(this, this.props.location.state)
        }.bind(this), 1000 * (KeePass4Web.getSettings().interval || 10 * 60))
    }

    componentWillUnmount() {
        if (this.timerId)
            clearInterval(this.timerId)

        this.abortRequests()
    }
}

