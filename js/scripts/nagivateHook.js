import React from 'react'
import {useLocation, useNavigate} from 'react-router-dom'

const withNavigateHook = Component => props => {
    const navigate = useNavigate()
    const location = useLocation()
    // other hooks

    return (
        <Component
            {...props}
            {...{navigate, location}}
        />
    );
};

export default withNavigateHook