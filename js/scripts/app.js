import jQuery from 'jquery'
import '../style/app.css'
import React from 'react'
import ReactDOM from 'react-dom'

import {BrowserRouter, Route, Routes} from 'react-router-dom'

import UserLogin from './UserLogin'
import BackendLogin from './BackendLogin'
import DBLogin from './DBLogin'
import Viewport from "./Viewport"


// global namespace
window.KeePass4Web = {}

KeePass4Web.checkAuth = function (state, callback) {
    return KeePass4Web.ajax('authenticated', {
        method: "GET",
        success: callback,
        error: function (r, s, e) {
            if (r.status != 200 && r.status != 401)
                return KeePass4Web.error(r, s, e)

            let authData = r.responseJSON && r.responseJSON.data
            state.redirect = true

            // route to proper login page if unauthenticated
            // in that order
            if (!authData) {
                KeePass4Web.clearStorage()
                this.props.navigate('/user_login', state)
            } else if (authData.user) {
                let user = authData.user
                if (user.type === 'redirect') {
                    window.location = user.url
                    // stopping javascript execution to prevent redirect loop
                    throw 'Redirecting'
                } else if (user.type === 'mask') {
                    this.props.navigate('/user_login', state)
                } else
                    alert("unknown login type")
            } else if (!authData.backend) {
                // TODO: Don't redirect to backend if db is open
                let template = KeePass4Web.getSettings().template
                if (template.type === 'redirect') {
                    window.location = template.url
                    throw 'Redirecting'
                } else if (template.type === 'mask')
                    this.props.navigate('/user_login', state)
            } else if (!authData.db) {
                this.props.navigate('/db_login', state)
            }

            return false
        }.bind(this),
    })
}

// simple wrapper for ajax calls, in case implementation changes
KeePass4Web.ajax = function (url, conf) {
    conf.url = `api/v1/${url}`

    // set defaults
    conf.method = typeof conf.method === 'undefined' ? 'POST' : conf.method
    conf.dataType = typeof conf.dataType === 'undefined' ? 'json' : conf.dataType

    if (typeof conf.headers === 'undefined') {
        conf.headers = {}
    }
    conf.headers['X-CSRF-Token'] = KeePass4Web.getCSRFToken()

    KeePass4Web.restartTimer(true)
    return jQuery.ajax(conf)
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

KeePass4Web.error = function (r, s, e) {
    // ignore aborted requests
    if (e === 'abort')
        return
    if (r.status == 401) {
        if (this.props.navigate) {
            // redirect first, to hide sensitive data
            this.props.navigate('/db_login', {replace: true})
            this.props.navigate('/', {
                replace: true,
                info: 'Session expired'
            })
        } else {
            alert('Your session expired')
            window.location.reload()
        }
    } else {
        let error = e
        if (r.responseJSON)
            error = r.responseJSON.message
        // disable remaining loading masks
        if (this.state) {
            this.setState({
                groupMask: false,
                nodeMask: false,
            })
        }
        alert(error)
    }
}


ReactDOM.render(
    <BrowserRouter>
        <Routes>
            <Route path="/" index Component={Viewport}/>
            <Route path="/user_login" Component={UserLogin}/>
            <Route path="/backend_login" Component={BackendLogin}/>
            <Route path="/db_login" Component={DBLogin}/>
        </Routes>
    </BrowserRouter>,
    document.getElementById('app-content')
)
