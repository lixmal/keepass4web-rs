import '../style/app.css'
import React from 'react'
import ReactDOM from 'react-dom'

import {BrowserRouter, Route, Routes} from 'react-router-dom'

import Splash from "./Splash"
import Viewport from "./Viewport"
import UserLogin from './UserLogin'
import BackendLogin from './BackendLogin'
import DBLogin from './DBLogin'
import CallbackUserAuth from './CallbackUserAuth'
import HTTPError from "./HTTPError"

// global namespace
window.KeePass4Web = {}

KeePass4Web.checkAuth = function (state) {
    return KeePass4Web.fetch('authenticated', {
        method: "GET",
        success: function () {
            this.props.navigate('/keepass', {replace: true})
        }.bind(this),
        error: function (error) {
            if (error.name === 'AbortError')
                return

            if (error instanceof HTTPError) {
                if (error.status !== 401)
                    return KeePass4Web.error(error)
            } else {
                return KeePass4Web.error(error)
            }

            let authData = error.data

            // route to proper login page if unauthenticated
            // in that order
            if (!authData) {
                KeePass4Web.clearStorage()
                this.props.navigate('/user_login', {state: state, redirect: true})
            } else if (authData.user) {
                let user = authData.user
                if (user.type === 'redirect') {
                    window.location = user.url
                    // stopping javascript execution to prevent redirect loop
                    throw 'Redirecting'
                } else if (user.type === 'mask') {
                    this.props.navigate('/user_login', {state: state, redirect: true})
                } else if (user.type === 'none') {
                    if (!state) state = {}
                    state.no_login = true
                    this.props.navigate('/user_login', {state: state, redirect: true})
                } else
                    alert("unknown login type")
            } else if (!authData.backend) {
                // TODO: Don't redirect to backend if db is open
                let template = KeePass4Web.getSettings().template
                if (template.type === 'redirect') {
                    window.location = template.url
                    throw 'Redirecting'
                } else if (template.type === 'mask')
                    this.props.navigate('/user_login', {state: state, redirect: true})
            } else if (!authData.db) {
                this.props.navigate('/db_login', {state: state, redirect: true})
            }
        }.bind(this),
    })
}

// simple wrapper for ajax calls, in case implementation changes
KeePass4Web.fetch = function (url, conf) {
    url = `api/v1/${url}`

    // set defaults
    if (typeof conf.method === 'undefined')
        conf.method = "POST"

    conf.headers = {
        'Accept': 'application/json',
    }

    const csrf_token = KeePass4Web.getCSRFToken()
    if (csrf_token)
        conf.headers['X-CSRF-Token'] = csrf_token

    const controller = new AbortController()
    conf.signal = controller.signal
    KeePass4Web.restartTimer(true)


    if (conf.data) {
        let params = new URLSearchParams(Object.entries(conf.data)).toString();
        if (conf.method === "GET") {
            url = `${url}?${params}`
        } else {
            conf.headers['Content-Type'] = 'application/x-www-form-urlencoded'
            conf.body = params
        }
    }

    fetch(url, conf).then(async function (response) {
        let message, data
        try {
            let json = await response.clone().json()
            message = json.message
            data = json.data
        } catch (e) {
            try {
                message = await response.text()
            } catch (e) {
                message = new Error("failed to read reponse")
            }
        }

        if (!response.ok) {
            throw new HTTPError(response, message, data)
        }

        conf.success && conf.success(data)
    }).catch(conf.error).finally(conf.complete)

    return controller
}

// leave room for implementation changes
KeePass4Web.clearStorage = function () {
    localStorage.removeItem('settings')
    localStorage.removeItem('CSRFToken')
}

KeePass4Web.setCSRFToken = function (CSRFToken) {
    localStorage.setItem('CSRFToken', CSRFToken || '')
}

KeePass4Web.getCSRFToken = function () {
    return localStorage.getItem('CSRFToken') || null
}

KeePass4Web.setSettings = function (settings) {
    const stored = KeePass4Web.getSettings()
    for (const k in settings) {
        stored[k] = settings[k]
    }
    localStorage.setItem('settings', JSON.stringify(stored))
}

KeePass4Web.getSettings = function () {
    const settings = localStorage.getItem('settings')
    if (settings)
        return JSON.parse(settings)
    return {}
}

KeePass4Web.timer = false
KeePass4Web.restartTimer = function (val) {
    if (typeof val !== 'undefined') KeePass4Web.timer = val
    return KeePass4Web.timer
}

KeePass4Web.error = function (error) {
    // ignore aborted requests
    if (error.name === 'AbortError')
        return

    if (error instanceof HTTPError && error.status === 401) {
        if (this.props.navigate) {
            this.props.navigate('/', {
                state: {
                    info: 'Session expired'
                },
                replace: true,
            })
        } else {
            alert('The session expired')
            window.location = '/'
        }
        return

    }

    // disable remaining loading masks
    if (this.state && typeof this.state.error !== 'undefined') {
        this.setState({
            error: error.toString()
        })
    } else {
        alert(error.toString())
    }
}


ReactDOM.render(
    <BrowserRouter>
        <Routes>
            <Route path="/" index Component={Splash}/>
            <Route path="/keepass" Component={Viewport}/>
            <Route path="/user_login" Component={UserLogin}/>
            <Route path="/backend_login" Component={BackendLogin}/>
            <Route path="/db_login" Component={DBLogin}/>
            <Route path="/callback_user_auth" Component={CallbackUserAuth}/>
        </Routes>
    </BrowserRouter>,
    document.getElementById('app-content')
)
